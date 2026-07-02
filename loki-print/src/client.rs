// SPDX-License-Identifier: Apache-2.0

//! The blocking IPP client: submit a rendered PDF, poll the job.

use std::time::{Duration, Instant};

use ipp::attribute::IppAttribute;
use ipp::model::{DelimiterTag, JobState};
use ipp::operation::builder::IppOperationBuilder;
use ipp::payload::IppPayload;
use ipp::prelude::Uri;
use ipp::request::IppRequestResponse;
use ipp::value::IppValue;

use crate::error::PrintError;
use crate::options::PrintOptions;

/// Poll cadence while waiting for a job to finish.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Job lifecycle states (mirrors IPP `job-state`, RFC 8011 §5.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintJobState {
    /// Queued.
    Pending,
    /// Held by the printer (e.g. waiting for media or authentication).
    Held,
    /// Printing.
    Processing,
    /// Paused by the printer.
    Stopped,
    /// Canceled by an operator or user.
    Canceled,
    /// Aborted by the printer.
    Aborted,
    /// Finished successfully.
    Completed,
    /// The printer reported a state outside RFC 8011.
    Unknown,
}

impl PrintJobState {
    fn from_ipp(state: JobState) -> Self {
        match state {
            JobState::Pending => Self::Pending,
            JobState::PendingHeld => Self::Held,
            JobState::Processing => Self::Processing,
            JobState::ProcessingStopped => Self::Stopped,
            JobState::Canceled => Self::Canceled,
            JobState::Aborted => Self::Aborted,
            JobState::Completed => Self::Completed,
        }
    }

    /// Whether the job can make no further progress.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Canceled | Self::Aborted | Self::Completed)
    }
}

/// A connection to one IPP printer (or a CUPS queue, which speaks IPP).
pub struct IppPrinter {
    uri: Uri,
    client: ipp::client::blocking::IppClient,
}

impl IppPrinter {
    /// Connects to a printer URI (`ipp://host/ipp/print`, `http://…:631/…`).
    pub fn connect(uri: &str) -> Result<Self, PrintError> {
        let parsed: Uri = uri.parse().map_err(|e| PrintError::InvalidPrinterUri {
            uri: uri.to_owned(),
            reason: format!("{e}"),
        })?;
        Ok(Self {
            uri: parsed.clone(),
            client: ipp::client::blocking::IppClient::new(parsed),
        })
    }

    /// Submits a rendered PDF as a Print-Job; returns the printer's job id.
    pub fn print_pdf(&self, pdf: Vec<u8>, options: &PrintOptions) -> Result<i32, PrintError> {
        let payload = IppPayload::new(std::io::Cursor::new(pdf));
        let mut builder = IppOperationBuilder::print_job(self.uri.clone(), payload)
            .user_name("loki-headless")
            .job_title(options.job_title.as_deref().unwrap_or("loki document"));
        builder = builder.attribute(
            IppAttribute::with_name(
                IppAttribute::DOCUMENT_FORMAT,
                IppValue::MimeMediaType(
                    "application/pdf"
                        .try_into()
                        .map_err(|_| PrintError::InvalidOption("document-format".into()))?,
                ),
            )
            .map_err(|_| PrintError::InvalidOption("document-format".into()))?,
        );
        for attribute in options.ipp_attributes()? {
            builder = builder.attribute(attribute);
        }
        let operation = builder
            .build()
            .map_err(|e| PrintError::InvalidOption(e.to_string()))?;
        let response = self.client.send(operation)?;
        Self::check_status(&response)?;
        Self::job_id_of(&response).ok_or(PrintError::MissingJobId)
    }

    /// Queries the current state of a submitted job.
    pub fn job_state(&self, job_id: i32) -> Result<PrintJobState, PrintError> {
        let operation = IppOperationBuilder::get_job_attributes(self.uri.clone(), job_id)
            .build()
            .map_err(|e| PrintError::InvalidOption(e.to_string()))?;
        let response = self.client.send(operation)?;
        Self::check_status(&response)?;
        Ok(Self::job_state_of(&response).unwrap_or(PrintJobState::Unknown))
    }

    /// Polls until the job reaches a terminal state or `timeout` elapses.
    /// Completion (and failure) reporting to the audit log is the caller's
    /// responsibility (ADR-C023).
    pub fn wait_for_completion(
        &self,
        job_id: i32,
        timeout: Duration,
    ) -> Result<PrintJobState, PrintError> {
        let deadline = Instant::now() + timeout;
        loop {
            let state = self.job_state(job_id)?;
            match state {
                PrintJobState::Completed => return Ok(state),
                PrintJobState::Canceled | PrintJobState::Aborted => {
                    return Err(PrintError::JobFailed { job_id, state });
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(PrintError::JobTimeout {
                    job_id,
                    timeout_secs: timeout.as_secs(),
                });
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    fn check_status(response: &IppRequestResponse) -> Result<(), PrintError> {
        let status = response.header().status_code();
        if status.is_success() {
            Ok(())
        } else {
            Err(PrintError::PrinterStatus {
                status: status as u16,
            })
        }
    }

    fn job_id_of(response: &IppRequestResponse) -> Option<i32> {
        Self::job_attribute(response, IppAttribute::JOB_ID).and_then(|value| match value {
            IppValue::Integer(id) => Some(*id),
            _ => None,
        })
    }

    fn job_state_of(response: &IppRequestResponse) -> Option<PrintJobState> {
        Self::job_attribute(response, IppAttribute::JOB_STATE).and_then(|value| match value {
            IppValue::Enum(raw) => {
                let state = JobState::try_from(*raw).ok()?;
                Some(PrintJobState::from_ipp(state))
            }
            _ => None,
        })
    }

    fn job_attribute<'a>(response: &'a IppRequestResponse, name: &str) -> Option<&'a IppValue> {
        response
            .attributes()
            .groups_of(DelimiterTag::JobAttributes)
            .flat_map(|group| group.attributes().values())
            .find(|attribute| attribute.name().as_ref() == name)
            .map(IppAttribute::value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states() {
        assert!(PrintJobState::Completed.is_terminal());
        assert!(PrintJobState::Aborted.is_terminal());
        assert!(PrintJobState::Canceled.is_terminal());
        assert!(!PrintJobState::Processing.is_terminal());
        assert!(!PrintJobState::Held.is_terminal());
    }

    #[test]
    fn bad_uri_is_a_typed_error() {
        assert!(matches!(
            IppPrinter::connect("not a uri"),
            Err(PrintError::InvalidPrinterUri { .. })
        ));
        assert!(IppPrinter::connect("ipp://printer.local/ipp/print").is_ok());
    }

    #[test]
    fn ipp_job_states_map_completely() {
        for (ipp, ours) in [
            (JobState::Pending, PrintJobState::Pending),
            (JobState::PendingHeld, PrintJobState::Held),
            (JobState::Processing, PrintJobState::Processing),
            (JobState::ProcessingStopped, PrintJobState::Stopped),
            (JobState::Canceled, PrintJobState::Canceled),
            (JobState::Aborted, PrintJobState::Aborted),
            (JobState::Completed, PrintJobState::Completed),
        ] {
            assert_eq!(PrintJobState::from_ipp(ipp), ours);
        }
    }
}

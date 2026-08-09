use appthere_color::{
    GamutWarning, IccProfile, ProofingConfig, RenderingIntent, RgbColor,
};

#[test]
fn proofing_srgb_through_srgb() {
    let srgb = IccProfile::new_srgb();

    let config = ProofingConfig::builder()
        .source(&srgb)
        .simulation(&srgb)
        .display(&srgb)
        .simulation_intent(RenderingIntent::Perceptual)
        .display_intent(RenderingIntent::RelativeColorimetric)
        .build()
        .unwrap();

    let input = [0.5_f32, 0.3, 0.8];
    let mut output = [0.0_f32; 3];
    config.proof(&input, &mut output).unwrap();

    // sRGB → sRGB → sRGB should be nearly identity
    let tolerance = 1.0 / 255.0;
    for (inp, out) in input.iter().zip(output.iter()) {
        assert!(
            (inp - out).abs() <= tolerance,
            "sRGB round-trip delta too large: inp={inp}, out={out}"
        );
    }
}

#[test]
fn proofing_missing_source_fails() {
    let srgb = IccProfile::new_srgb();

    let result = ProofingConfig::builder()
        .simulation(&srgb)
        .display(&srgb)
        .build();

    assert!(result.is_err());
}

#[test]
fn proofing_missing_simulation_fails() {
    let srgb = IccProfile::new_srgb();

    let result = ProofingConfig::builder()
        .source(&srgb)
        .display(&srgb)
        .build();

    assert!(result.is_err());
}

#[test]
fn proofing_missing_display_fails() {
    let srgb = IccProfile::new_srgb();

    let result = ProofingConfig::builder()
        .source(&srgb)
        .simulation(&srgb)
        .build();

    assert!(result.is_err());
}

#[test]
fn proofing_gamut_warning_default() {
    let srgb = IccProfile::new_srgb();

    let config = ProofingConfig::builder()
        .source(&srgb)
        .simulation(&srgb)
        .display(&srgb)
        .build()
        .unwrap();

    assert_eq!(config.gamut_warning(), GamutWarning::None);
}

#[test]
fn proofing_gamut_warning_replace() {
    let srgb = IccProfile::new_srgb();
    let warning_color = RgbColor::new(1.0, 0.0, 1.0);

    let config = ProofingConfig::builder()
        .source(&srgb)
        .simulation(&srgb)
        .display(&srgb)
        .gamut_warning(GamutWarning::Replace(warning_color))
        .build()
        .unwrap();

    assert_eq!(
        config.gamut_warning(),
        GamutWarning::Replace(warning_color)
    );
}

#[test]
fn proofing_channel_counts() {
    let srgb = IccProfile::new_srgb();

    let config = ProofingConfig::builder()
        .source(&srgb)
        .simulation(&srgb)
        .display(&srgb)
        .build()
        .unwrap();

    assert_eq!(config.source_channels(), 3);
    assert_eq!(config.destination_channels(), 3);
}

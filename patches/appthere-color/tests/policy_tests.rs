use appthere_color::{
    ColorPolicy, IccProfile, OutputIntent, RenderingIntent,
};

#[test]
fn policy_builder_defaults() {
    let policy = ColorPolicy::builder().build();

    assert!(policy.output_intent().is_none());
    assert!(policy.fallback_profile().is_none());
    assert_eq!(policy.default_intent(), RenderingIntent::Perceptual);
}

#[test]
fn policy_with_output_intent() {
    let srgb = IccProfile::new_srgb();
    let oi = OutputIntent::new("sRGB IEC61966-2.1".to_string(), srgb);

    let policy = ColorPolicy::builder()
        .output_intent(oi)
        .build();

    let intent = policy.output_intent().unwrap();
    assert_eq!(intent.name(), "sRGB IEC61966-2.1");
}

#[test]
fn policy_with_fallback_profile() {
    let srgb = IccProfile::new_srgb();

    let policy = ColorPolicy::builder()
        .fallback_profile(srgb)
        .build();

    assert!(policy.fallback_profile().is_some());
}

#[test]
fn policy_custom_intent() {
    let policy = ColorPolicy::builder()
        .default_intent(RenderingIntent::AbsoluteColorimetric)
        .build();

    assert_eq!(
        policy.default_intent(),
        RenderingIntent::AbsoluteColorimetric
    );
}

#[test]
fn policy_embedded_profile_registry() {
    let srgb = IccProfile::new_srgb();
    let adobe = IccProfile::new_adobe_rgb();

    let policy = ColorPolicy::builder()
        .embed_profile("sRGB".to_string(), srgb)
        .embed_profile("AdobeRGB".to_string(), adobe)
        .build();

    assert!(policy.get_profile("sRGB").is_some());
    assert!(policy.get_profile("AdobeRGB").is_some());
    assert!(policy.get_profile("nonexistent").is_none());
}

#[test]
fn output_intent_with_condition() {
    let srgb = IccProfile::new_srgb();
    let oi = OutputIntent::new("FOGRA39".to_string(), srgb)
        .with_condition("FOGRA39L".to_string());

    assert_eq!(oi.name(), "FOGRA39");
    assert_eq!(oi.condition(), Some("FOGRA39L"));
}

#[test]
fn output_intent_condition_none_by_default() {
    let srgb = IccProfile::new_srgb();
    let oi = OutputIntent::new("test".to_string(), srgb);

    assert_eq!(oi.condition(), None);
}

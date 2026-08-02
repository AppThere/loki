use appthere_color::{ColorTransform, IccProfile, RenderingIntent};

#[test]
fn srgb_to_adobe_rgb_round_trip() {
    let srgb = IccProfile::new_srgb();
    let adobe = IccProfile::new_adobe_rgb();

    let forward = ColorTransform::builder()
        .source(&srgb)
        .destination(&adobe)
        .intent(RenderingIntent::RelativeColorimetric)
        .build()
        .unwrap();

    let backward = ColorTransform::builder()
        .source(&adobe)
        .destination(&srgb)
        .intent(RenderingIntent::RelativeColorimetric)
        .build()
        .unwrap();

    let original = [0.8_f32, 0.3, 0.5];
    let mut intermediate = [0.0_f32; 3];
    let mut round_trip = [0.0_f32; 3];

    forward.transform(&original, &mut intermediate).unwrap();
    backward.transform(&intermediate, &mut round_trip).unwrap();

    // Round-trip delta must be within 1/255 per channel for 8-bit equivalence
    let tolerance = 1.0 / 255.0;
    for (orig, rt) in original.iter().zip(round_trip.iter()) {
        assert!(
            (orig - rt).abs() <= tolerance,
            "round-trip delta too large: orig={orig}, rt={rt}, delta={}",
            (orig - rt).abs()
        );
    }
}

#[test]
fn srgb_to_display_p3_transform() {
    let srgb = IccProfile::new_srgb();
    let p3 = IccProfile::new_display_p3();

    let xform = ColorTransform::builder()
        .source(&srgb)
        .destination(&p3)
        .intent(RenderingIntent::Perceptual)
        .build()
        .unwrap();

    let input = [0.5_f32, 0.5, 0.5];
    let mut output = [0.0_f32; 3];
    xform.transform(&input, &mut output).unwrap();

    // Mid-gray should remain close to mid-gray
    let tolerance = 0.05;
    for val in &output {
        assert!(
            (*val - 0.5).abs() < tolerance,
            "mid-gray shifted too far: {val}"
        );
    }
}

#[test]
fn transform_channel_counts() {
    let srgb = IccProfile::new_srgb();
    let adobe = IccProfile::new_adobe_rgb();

    let xform = ColorTransform::builder()
        .source(&srgb)
        .destination(&adobe)
        .build()
        .unwrap();

    assert_eq!(xform.source_channels(), 3);
    assert_eq!(xform.destination_channels(), 3);
}

#[test]
fn all_rendering_intents_build() {
    let srgb = IccProfile::new_srgb();
    let adobe = IccProfile::new_adobe_rgb();

    let intents = [
        RenderingIntent::Perceptual,
        RenderingIntent::RelativeColorimetric,
        RenderingIntent::Saturation,
        RenderingIntent::AbsoluteColorimetric,
    ];

    for intent in intents {
        let xform = ColorTransform::builder()
            .source(&srgb)
            .destination(&adobe)
            .intent(intent)
            .build();
        assert!(xform.is_ok(), "failed to build with intent {:?}", intent);
    }
}

#[test]
fn gray_to_srgb_transform() {
    let gray = IccProfile::new_gray(2.2);
    let srgb = IccProfile::new_srgb();

    let xform = ColorTransform::builder()
        .source(&gray)
        .destination(&srgb)
        .build()
        .unwrap();

    assert_eq!(xform.source_channels(), 1);
    assert_eq!(xform.destination_channels(), 3);

    // Pure white in gray should map to white in sRGB
    let input = [1.0_f32];
    let mut output = [0.0_f32; 3];
    xform.transform(&input, &mut output).unwrap();

    let tolerance = 0.05;
    for val in &output {
        assert!(
            (*val - 1.0).abs() < tolerance,
            "white gray should map to white sRGB: {val}"
        );
    }
}

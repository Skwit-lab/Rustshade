use rustshade_core::{effect_def, validate, EffectSpec, Preset};

fn spec(id: &str, params: &[(&str, f32)]) -> EffectSpec {
    let mut table = toml::Table::new();
    for (k, v) in params {
        table.insert(k.to_string(), toml::Value::Float(*v as f64));
    }
    EffectSpec {
        id: id.to_string(),
        enabled: true,
        params: table,
    }
}

#[test]
fn valid_preset_passes() {
    let preset = Preset {
        settings: Default::default(),
        effects: vec![
            spec("grade", &[("contrast", 0.1)]),
            spec("vignette", &[("amount", 0.7)]),
            spec("lut", &[]),
        ],
    };
    assert!(validate(&preset).is_ok());
}

#[test]
fn unknown_effect_is_rejected() {
    let preset = Preset {
        settings: Default::default(),
        effects: vec![spec("vibes", &[])],
    };
    assert!(validate(&preset).is_err());
}

#[test]
fn out_of_range_param_is_rejected() {
    let preset = Preset {
        settings: Default::default(),
        effects: vec![spec("grade", &[("exposure", 9.0)])],
    };
    let errs = validate(&preset).unwrap_err();
    assert_eq!(errs.len(), 1);
    assert!(format!("{}", errs[0]).contains("exposure"));
}

#[test]
fn unknown_param_is_rejected() {
    let preset = Preset {
        settings: Default::default(),
        effects: vec![spec("grain", &[("intensity", 0.2)])],
    };
    let errs = validate(&preset).unwrap_err();
    assert!(format!("{}", errs[0]).contains("intensity"));
}

#[test]
fn registry_defaults_are_in_range() {
    for def in rustshade_core::REGISTRY {
        for p in def.params {
            assert!(
                p.default >= p.min && p.default <= p.max,
                "default of {}.{} out of range",
                def.id,
                p.name
            );
        }
    }
}

#[test]
fn built_in_default_preset_is_valid() {
    let preset = Preset::default_preset();
    validate(&preset).expect("built-in preset must be valid");
    assert!(!preset.effects.is_empty());
}

#[test]
fn lut_param_string_type() {
    let def = effect_def("lut").unwrap();
    assert!(def.params.iter().any(|p| p.name == "path"));
}

#[test]
fn empty_toml_is_empty_preset() {
    let preset = Preset::parse("").unwrap();
    assert!(preset.effects.is_empty());
}

//! Electron's `muse-wonder-is-offered.test.ts` (2 cases): Muse Code's CLI lists no models, so Locust offers the two
//! Meta serves, newest first, each with Muse Code's own eight efforts.
use super::*;

// case: are Muse Wonder 1.0 first, then Muse Spark 1.3, each with Muse Code's own eight efforts at its default
#[test]
fn are_muse_wonder_first_then_muse_spark_each_with_eight_efforts_at_its_default() {
    let models = muse_models();
    let named: Vec<(&str, &str)> = models.iter().map(|m| (m["id"].as_str().unwrap(), m["displayName"].as_str().unwrap())).collect();
    assert_eq!(named, [("muse-wonder-1.0", "Muse Wonder 1.0"), ("muse-spark-1.3", "Muse Spark 1.3")]);
    for model in &models {
        assert_eq!(model["runtime"], "muse");
        assert_eq!(model["supportedEfforts"], json!(["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"]));
        assert_eq!(model["defaultEffort"], "high");
    }
}

// case: are offered only when Muse Code is ready here
#[test]
fn are_offered_only_when_muse_code_is_here() {
    let muse_ids = |listed: &Listed| -> Vec<String> {
        catalog(listed).iter().filter(|m| m["runtime"] == "muse").filter_map(|m| m["id"].as_str().map(str::to_owned)).collect()
    };
    assert!(muse_ids(&Listed::default()).is_empty());
    let here = muse_ids(&Listed { muse: true, ..Listed::default() });
    assert!(here.contains(&"muse-wonder-1.0".to_owned()) && here.contains(&"muse-spark-1.3".to_owned()), "{here:?}");
}

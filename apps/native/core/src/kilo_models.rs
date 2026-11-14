/// Kilo model IDs, descriptions, display names, and start-model selection carried from
/// `apps/desktop/src/shared/kilo-models.ts`.

/// Auto Free model ID routed through Kilo.
pub const KILO_AUTO_FREE_MODEL: &str = crate::runtime::kilo::AUTO_FREE_MODEL;

/// Runtime and model ID pair for checking whether Auto Free is listed for Kilo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KiloListedModel<'a> {
    pub runtime: &'a str,
    pub id: &'a str,
}

/// Short description shown for a Kilo model ID, highlighting Auto Free's logging tradeoff.
pub fn kilo_model_description(id: &str) -> &'static str {
    if id == KILO_AUTO_FREE_MODEL {
        return "Free · no sign-in. Auto Free may route requests to providers that log prompts and outputs and use them to improve their services. Do not submit personal or confidential data.";
    }
    if crate::invariants::regex(r"(?i)(?:^|[-/:])free$").is_match(id) {
        return "Free · no sign-in";
    }
    "Through Kilo. A paid model needs a sign-in."
}

/// Display name for a Kilo model without the repeated `Kilo Gateway/` prefix.
pub fn kilo_model_name(id: &str, name: &str) -> String {
    let bare = crate::invariants::regex(r"(?i)^Kilo Gateway/").replace(name, "");
    if crate::invariants::regex(r"(?i)^kilo/kilo-").is_match(id)
        && !crate::invariants::regex(r"(?i)^kilo ").is_match(&bare)
    {
        format!("Kilo {bare}")
    } else {
        bare.into_owned()
    }
}

/// Moves a new Kilo conversation off `account-default` once Auto Free is listed for Kilo.
pub fn kilo_start_model(
    runtime: &str,
    model: &str,
    listed: &[KiloListedModel<'_>],
) -> Option<&'static str> {
    if runtime != "kilo" || model != "account-default" {
        return None;
    }
    listed
        .iter()
        .any(|entry| entry.runtime == "kilo" && entry.id == KILO_AUTO_FREE_MODEL)
        .then_some(KILO_AUTO_FREE_MODEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_same_auto_free_id_the_run_starts_on() {
        assert_eq!(KILO_AUTO_FREE_MODEL, crate::runtime::kilo::AUTO_FREE_MODEL);
        assert_eq!(KILO_AUTO_FREE_MODEL, "kilo/kilo-auto/free");
    }

    #[test]
    fn says_the_privacy_cost_on_auto_free_only() {
        let auto_free = kilo_model_description(KILO_AUTO_FREE_MODEL);
        assert!(auto_free.contains("Do not submit personal or confidential data"));
        assert!(auto_free.contains("log prompts and outputs"));
        assert_eq!(
            kilo_model_description("kilo/other/model:free"),
            "Free · no sign-in"
        );
        assert_eq!(
            kilo_model_description("vendor/model-free"),
            "Free · no sign-in"
        );
        assert_eq!(
            kilo_model_description("kilo/google/gemini-3-pro-image"),
            "Through Kilo. A paid model needs a sign-in."
        );
        assert!(!kilo_model_description("kilo/free-tier-preview").contains("confidential"));
    }

    #[test]
    fn moves_a_new_conversation_off_the_account_default_once_auto_free_is_listed() {
        let listed = [KiloListedModel {
            runtime: "kilo",
            id: KILO_AUTO_FREE_MODEL,
        }];
        assert_eq!(
            kilo_start_model("kilo", "account-default", &listed),
            Some(KILO_AUTO_FREE_MODEL)
        );
        assert_eq!(kilo_start_model("kilo", "account-default", &[]), None);
        assert_eq!(
            kilo_start_model("kilo", "kilo/google/gemini-3-pro-image", &listed),
            None
        );
        assert_eq!(
            kilo_start_model("opencode", "account-default", &listed),
            None
        );
        assert_eq!(
            kilo_start_model(
                "kilo",
                "account-default",
                &[KiloListedModel {
                    runtime: "opencode",
                    id: KILO_AUTO_FREE_MODEL,
                }]
            ),
            None
        );
    }

    #[test]
    fn names_a_model_without_the_gateway_every_kilo_row_would_repeat() {
        assert_eq!(
            kilo_model_name(
                "kilo/nvidia/nemotron-3.5-lightning:free",
                "Kilo Gateway/NVIDIA: Nemotron 3.5 Lightning (free)"
            ),
            "NVIDIA: Nemotron 3.5 Lightning (free)"
        );
        assert_eq!(
            kilo_model_name(
                "anthropic/claude-sonnet-4.5",
                "Anthropic/Claude Sonnet 4.5"
            ),
            "Anthropic/Claude Sonnet 4.5"
        );
    }

    #[test]
    fn keeps_kilos_word_on_its_own_router_so_the_chip_never_says_a_bare_auto() {
        assert_eq!(
            kilo_model_name(KILO_AUTO_FREE_MODEL, "Kilo Gateway/Auto Free"),
            "Kilo Auto Free"
        );
        assert_eq!(
            kilo_model_name("kilo/kilo-auto/frontier", "Kilo Gateway/Auto Frontier"),
            "Kilo Auto Frontier"
        );
        assert_eq!(
            kilo_model_name("kilo/kilo-auto/small", "Kilo Gateway/Kilo Auto Small"),
            "Kilo Auto Small"
        );
    }
}

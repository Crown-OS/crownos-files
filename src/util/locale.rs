use std::env;

const LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

/// The locale suffixes a translated desktop-entry key is tried with, most
/// specific first, for `lang_COUNTRY.ENCODING@MODIFIER`.
pub fn message_locales() -> Vec<String> {
    LOCALE_VARIABLES
        .iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
        .map(|value| locale_fallbacks(&value))
        .unwrap_or_default()
}

pub fn locale_fallbacks(locale: &str) -> Vec<String> {
    let (rest, modifier) = locale
        .split_once('@')
        .map_or((locale, None), |(rest, modifier)| (rest, Some(modifier)));
    let rest = rest.split_once('.').map_or(rest, |(rest, _)| rest);
    let (language, country) = rest
        .split_once('_')
        .map_or((rest, None), |(language, country)| {
            (language, Some(country))
        });
    if language.is_empty() || language == "C" || language == "POSIX" {
        return Vec::new();
    }
    let mut candidates = Vec::with_capacity(4);
    if let (Some(country), Some(modifier)) = (country, modifier) {
        candidates.push(format!("{language}_{country}@{modifier}"));
    }
    if let Some(country) = country {
        candidates.push(format!("{language}_{country}"));
    }
    if let Some(modifier) = modifier {
        candidates.push(format!("{language}@{modifier}"));
    }
    candidates.push(language.to_owned());
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallbacks_go_from_specific_to_language() {
        assert_eq!(
            locale_fallbacks("sr_RS.UTF-8@latin"),
            ["sr_RS@latin", "sr_RS", "sr@latin", "sr"]
        );
        assert_eq!(locale_fallbacks("de_DE.UTF-8"), ["de_DE", "de"]);
        assert!(locale_fallbacks("C.UTF-8").is_empty());
    }
}

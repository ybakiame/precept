//! Exhaustive UI catalog. Repository/user values and protocol evidence are never translated.

macro_rules! msg_keys {
    ($($key:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Msg { $($key),* }
        impl Msg { pub const ALL: &'static [Self] = &[$(Self::$key),*]; }
    };
}

macro_rules! text_catalog {
    ($($key:ident => $value:literal),* $(,)?) => {
        pub(super) fn text(message: Msg) -> &'static str {
            match message { $(Msg::$key => $value,)* }
        }
    };
}

mod en;
mod types;
mod zh_cn;

pub use types::{Locale, Msg};

pub fn tr(locale: Locale, message: Msg) -> &'static str {
    match locale {
        Locale::En => en::text(message),
        Locale::ZhCn => zh_cn::text(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_catalogs_are_complete_and_controls_are_not_in_translations() {
        for locale in [Locale::En, Locale::ZhCn] {
            for key in Msg::ALL {
                let text = tr(locale, *key);
                assert!(!text.is_empty());
                assert!(!text.chars().any(char::is_control));
            }
        }
        assert_eq!(Locale::parse("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("../../profile"), None);
    }
}

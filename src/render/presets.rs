//! Ready-made sets of field values: a theme left with empty fields draws one of these at random
//! instead of always the same hint, so the same theme never prints the same ticket twice. The pick
//! comes from the ticket's seed (`Ctx::seed`), so the same text still gives the same ticket and a
//! new run gives another set.
//!
//! A preset only writes in fields the user left empty; anything typed always wins (see
//! `Ctx::field`, `Ctx::field_or` and `Ctx::given`). The keys must be fields of the theme: a test
//! checks it, because a key that no field asks for would be dropped in silence.

use super::theme::Theme;
use super::draw::Rng;

/// One set of `(field key, value)` pairs, and nothing for a field it does not name.
pub type Preset = &'static [(&'static str, &'static str)];

/// Keeps this pick apart from the numbers, speckle and codes the theme draws from the same seed.
const SALT: u64 = 0x7472_6f6e_6963_7072; // "tronicpr"

/// The sets a theme can draw from, empty when it has none.
pub fn sets(theme: Theme) -> &'static [Preset] {
    match theme {
        Theme::Receipt => RECEIPT,
        Theme::Diner => DINER,
        Theme::Coupon => COUPON,
        Theme::Fine => FINE,
        Theme::Wanted => WANTED,
        Theme::Boarding | Theme::Flight => BOARDING,
        Theme::Horoscope => HOROSCOPE,
        Theme::Train => TRAIN,
        Theme::Complaint => COMPLAINT,
        Theme::Butthurt => BUTTHURT,
        Theme::Cerfa => CERFA,
        _ => &[],
    }
}

/// The set this ticket draws its empty fields from. Pure: the same seed, the same set.
pub fn preset(theme: Theme, seed: u64) -> Preset {
    let all = sets(theme);
    if all.is_empty() {
        return &[];
    }
    all[Rng::new(seed ^ SALT).below(all.len())]
}

// ---- the sets -------------------------------------------------------------------------------

// The shop is set in capitals by the theme; the address and the date stay as written.
const RECEIPT: &[Preset] = &[
    &[("store", "TOTAL RECALL GROCERY"), ("address", "3 Rue de la Migraine"), ("date", "04/03/2019")],
    &[("store", "HONESTLY FINE FOODS"), ("address", "Behind the bus shelter"), ("date", "12/10/2021")],
    &[("store", "LAST RESORT DELI"), ("address", "Aisle 6, near the sighs")],
    &[("store", "BIG COMFORT"), ("address", "Unit 4, Retail Park of Regret"), ("date", "29/02/2019")],
    &[("store", "THE CORNER SHOP"), ("address", "Open when it feels like it")],
];

const DINER: &[Preset] = &[
    &[("place", "The Last Drop"), ("table", "6"), ("server", "Dolores")],
    &[("place", "Café Regret"), ("table", "13"), ("server", "Big Ron")],
    &[("place", "The Soggy Spoon"), ("table", "4"), ("server", "Kevin, probably")],
    &[("place", "The Warm Fridge"), ("table", "9"), ("server", "Brenda")],
];

const COUPON: &[Preset] = &[
    &[("offer", "-90% ON REGRET"), ("code", "OOPS-90"), ("expires", "31/02")],
    &[("offer", "BUY 1 SIGH FREE"), ("code", "SIGH-2X"), ("expires", "while joy lasts")],
    &[("offer", "FREE CROISSANT*"), ("code", "CROISSANT"), ("expires", "see footnote 4")],
    &[("offer", "TWO NAPS FOR ONE"), ("code", "NAP-2FOR1"), ("expires", "lunchtime")],
];

const FINE: &[Preset] = &[
    &[("offender", "The self-checkout"), ("amount", "60.00"), ("number", "418")],
    &[("offender", "B. Reasonable"), ("amount", "275.00"), ("number", "004417")],
    &[("offender", "Unknown, in a hat"), ("amount", "15.50"), ("number", "099999")],
    &[("offender", "A. Pedestrian"), ("amount", "999.99"), ("number", "000001")],
];

// The name is set in capitals by the theme.
const WANTED: &[Preset] = &[
    &[("name", "The Last Biscuit"), ("reward", "ONE (1) COFFEE")],
    &[("name", "Monday Morning"), ("reward", "A FULL NIGHT'S SLEEP")],
    &[("name", "The Printer Driver"), ("reward", "A WORKING USB PORT")],
    &[("name", "My Old Phone Charger"), ("reward", "THREE PENCE")],
    &[("name", "The Unread Inbox"), ("reward", "FIVE MINUTES OF PEACE")],
];

// Two airports twenty minutes apart make a short hop look like a connection. The date and the
// class are left out on purpose: the pass keeps today's date and the class the theme makes up.
const BOARDING: &[Preset] = &[
    &[("from", "BRU"), ("to", "CRL"), ("name", "A. PERSON"), ("seat", "1A"), ("flight", "TR 0404"), ("gate", "B12")],
    &[("from", "LHR"), ("to", "LHR"), ("name", "M. TIGHT"), ("seat", "23F"), ("flight", "TR 1313"), ("gate", "A1")],
    &[("from", "CDG"), ("to", "ORY"), ("name", "J. LATE"), ("seat", "99Z"), ("flight", "TR 0000"), ("gate", "Z9")],
    &[("from", "AMS"), ("to", "AMS"), ("name", "THE TOWEL"), ("seat", "42"), ("flight", "TR 4242"), ("gate", "C3")],
];

// Signs the sky never had. A sign from the theme's own list gets its symbol back; "Doomscroll"
// is nobody's sign, so it brings its own.
const HOROSCOPE: &[Preset] = &[
    &[("sign", "Coffee")],
    &[("sign", "Unread Mail")],
    &[("sign", "Lost Luggage")],
    &[("sign", "Pirate")],
    &[("sign", "Doomscroll"), ("symbol", "\u{2622}")],
];

// French stations, short hops: the funniest trip is the one that takes ten minutes by foot. Every
// station stays on one line, so the ticket keeps one height whichever set is drawn.
const TRAIN: &[Preset] = &[
    &[("from", "PARIS GARE DE L'EST"), ("to", "PARIS GARE DU NORD"), ("seat", "1 / 1"), ("operator", "TGV"), ("train", "0001"), ("price", "199,00 EUR"), ("class", "1ère")],
    &[("from", "LILLE FLANDRES"), ("to", "LILLE EUROPE"), ("seat", "44 / 44"), ("operator", "OUIGO"), ("train", "9999"), ("price", "2,50 EUR"), ("class", "2de")],
    &[("from", "MARSEILLE ST CHARLES"), ("to", "MARSEILLE BLANCARDE"), ("seat", "7 / 12"), ("operator", "TGV"), ("train", "6120"), ("price", "1,00 EUR"), ("class", "1ère")],
    &[("from", "BORDEAUX ST JEAN"), ("to", "TALENCE"), ("seat", "2 / 3"), ("operator", "INTERCITES"), ("train", "0007"), ("price", "0,10 EUR"), ("class", "2de")],
];

// The tick boxes stay unticked and the closing line stays the theme's own: only the blanks are
// filled, with the sort of thing people write in them.
const COMPLAINT: &[Preset] = &[
    &[("name", "The self-checkout"), ("dept", "Dept. of Small Print")],
    &[("name", "A very tired parent"), ("dept", "Ministry of Bedtime")],
    &[("name", "Kevin from accounts"), ("dept", "Bureau of Hold Music")],
    &[("name", "A. Customer"), ("dept", "Office of Circular Replies")],
];

const BUTTHURT: &[Preset] = &[
    &[("user", "u/definitely_human"), ("provider", "a data centre, somewhere"), ("model", "GPT-4o (on a bad day)")],
    &[("user", "u/one_more_prompt"), ("provider", "the cloud (vague)"), ("model", "the quantized one")],
    &[("user", "u/surely_smarter"), ("provider", "Big Tech"), ("model", "the smaller model")],
    &[("user", "u/just_one_more"), ("provider", "their servers, probably"), ("model", "the one with good PR")],
    &[("user", "u/trust_me_bro"), ("provider", "a garage in Ohio"), ("model", "the free tier")],
];

// In French, like the form. The applicant asks the administration for permission to use a model.
const CERFA: &[Preset] = &[
    &[("name", "Jean-Michel Formulaire"), ("service", "Bureau des usages numériques"), ("model", "Synthèse-7 (version allégée)"), ("date", "03/03/2020")],
    &[("name", "Nadine Sans-Dossier"), ("service", "Cellule innovation et bon sens"), ("model", "Rédacteur-2 (bêta interne)"), ("date", "31/02/2024")],
    &[("name", "Robert Duplicata"), ("service", "Direction des outils incertains"), ("model", "Assistant-Ministériel 1.0"), ("date", "14/07/2019")],
    &[("name", "Martine Pliage"), ("service", "Service des essais prudents"), ("model", "Modèle maison (sans fiche)"), ("date", "01/01/1970")],
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every theme with presets: what the tests below sweep.
    fn covered() -> Vec<Theme> {
        Theme::BUILTIN.into_iter().filter(|t| !sets(*t).is_empty()).collect()
    }

    #[test]
    fn every_preset_key_is_a_field_of_its_theme() {
        for theme in covered() {
            for set in sets(theme) {
                assert!((3..=5).contains(&sets(theme).len()), "{}: {} sets, want 3 to 5", theme.id(), sets(theme).len());
                assert!(!set.is_empty(), "{}: an empty set", theme.id());
                for (key, value) in *set {
                    assert!(
                        theme.fields().iter().any(|f| f.key == *key),
                        "{}: no field is called {key}, so the value would be dropped",
                        theme.id()
                    );
                    assert!(!value.trim().is_empty(), "{}: {key} is empty", theme.id());
                    assert!(value.chars().count() <= 32, "{}: {key} = {value:?} is too long for the field", theme.id());
                }
                let mut keys: Vec<&str> = set.iter().map(|(k, _)| *k).collect();
                keys.sort_unstable();
                let n = keys.len();
                keys.dedup();
                assert_eq!(keys.len(), n, "{}: a key is set twice in one preset", theme.id());
            }
        }
        assert!(covered().len() >= 10, "the themes with something to fill in");
    }

    #[test]
    fn a_theme_without_presets_has_no_sets() {
        for theme in [Theme::Plain, Theme::Parchment, Theme::Terminal, Theme::Codes] {
            assert!(sets(theme).is_empty(), "{}", theme.id());
            for seed in 0..64 {
                assert!(preset(theme, seed).is_empty(), "{} at seed {seed}", theme.id());
            }
        }
    }

    #[test]
    fn the_same_seed_picks_the_same_set() {
        for theme in covered() {
            for seed in [0u64, 1, 42, 7_391, u64::MAX] {
                let a = preset(theme, seed);
                let b = preset(theme, seed);
                assert!(std::ptr::eq(a, b), "{} at seed {seed} picked two different sets", theme.id());
            }
        }
    }

    #[test]
    fn a_sweep_of_seeds_reaches_every_set_and_more_than_one() {
        for theme in covered() {
            let mut seen: Vec<Preset> = Vec::new();
            for seed in 0..512u64 {
                let p = preset(theme, seed);
                if !seen.iter().any(|s| std::ptr::eq(*s, p)) {
                    seen.push(p);
                }
            }
            assert_eq!(seen.len(), sets(theme).len(), "{}: some sets are never picked", theme.id());
            assert!(seen.len() > 1, "{}: only one set to draw", theme.id());
        }
    }
}

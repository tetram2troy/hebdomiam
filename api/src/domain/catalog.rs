//! Énumérations partagées avec la base (types enum PostgreSQL) et le front
//! (`shared/catalog.ts` : UNITS / AISLES et clés i18n units.* / aisles.*).
//! Garder synchronisés.

use serde::{Deserialize, Serialize};

macro_rules! db_enum {
    ($(#[$meta:meta])* $name:ident, $pg:literal, { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(type_name = $pg)]
        pub enum $name {
            $(
                #[serde(rename = $text)]
                #[sqlx(rename = $text)]
                $variant,
            )+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }

            pub fn parse(text: &str) -> Option<$name> {
                match text { $($text => Some($name::$variant),)+ _ => None }
            }
        }
    };
}

db_enum!(
    /// Unités normalisées (métrique + impérial/tasses, décision D6).
    Unit, "unit", {
        G => "g", Kg => "kg", Ml => "ml", L => "l", Oz => "oz", Lb => "lb",
        Cup => "cup", Tbsp => "tbsp", Tsp => "tsp", Unit => "unit", Pinch => "pinch",
    }
);

db_enum!(
    /// Rayons d'épicerie, dans l'ordre du magasin (tri de la liste).
    Aisle, "aisle", {
        Produce => "produce", Butcher => "butcher", Fish => "fish", Dairy => "dairy",
        Dry => "dry", Canned => "canned", Frozen => "frozen", Bakery => "bakery",
        Spices => "spices", Beverages => "beverages", Household => "household", Other => "other",
    }
);

db_enum!(Role, "user_role", { Admin => "admin", User => "user" });
db_enum!(Locale, "locale", { Fr => "fr", En => "en" });
db_enum!(UnitSystem, "unit_system", { Metric => "metric", Imperial => "imperial" });
db_enum!(PlanStatus, "meal_plan_status", { Draft => "draft", Validated => "validated", Cooked => "cooked" });
db_enum!(PlanRecipeStatus, "meal_plan_recipe_status", {
    Suggested => "suggested", Accepted => "accepted", Swapped => "swapped", Cooked => "cooked",
});
db_enum!(EmailTokenType, "email_token_type", { Verify => "verify", Reset => "reset", Invite => "invite" });

impl Aisle {
    /// Position dans l'ordre du magasin.
    pub fn order(self) -> usize {
        Aisle::ALL.iter().position(|a| *a == self).unwrap_or(usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialisation_en_minuscules() {
        assert_eq!(serde_json::to_string(&Unit::Tbsp).unwrap(), "\"tbsp\"");
        assert_eq!(serde_json::from_str::<Aisle>("\"canned\"").unwrap(), Aisle::Canned);
        assert!(serde_json::from_str::<Unit>("\"teaspoon\"").is_err());
    }

    #[test]
    fn ordre_des_rayons() {
        assert_eq!(Aisle::Produce.order(), 0);
        assert_eq!(Aisle::Other.order(), Aisle::ALL.len() - 1);
    }

    #[test]
    fn parse_et_as_str() {
        for unit in Unit::ALL {
            assert_eq!(Unit::parse(unit.as_str()), Some(*unit));
        }
    }
}

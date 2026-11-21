//! Normalisation d'unités (PRD F6, décision D6) — fonctions pures.
//! Les quantités agrégées sont stockées en unité de BASE (g / ml) ; l'affichage
//! convertit selon la préférence utilisateur (métrique ou impérial/tasses).

use super::catalog::{Unit, UnitSystem};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Mass,
    Volume,
    Count,
    Pinch,
    None,
}

impl Dimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::Mass => "mass",
            Dimension::Volume => "volume",
            Dimension::Count => "count",
            Dimension::Pinch => "pinch",
            Dimension::None => "none",
        }
    }
}

/// Facteur vers l'unité de base (g pour la masse, ml pour le volume).
/// Tasses canadiennes : 1 tasse = 250 ml.
fn to_base_factor(unit: Unit) -> Option<(Dimension, f64)> {
    match unit {
        Unit::G => Some((Dimension::Mass, 1.0)),
        Unit::Kg => Some((Dimension::Mass, 1000.0)),
        Unit::Oz => Some((Dimension::Mass, 28.35)),
        Unit::Lb => Some((Dimension::Mass, 453.592)),
        Unit::Ml => Some((Dimension::Volume, 1.0)),
        Unit::L => Some((Dimension::Volume, 1000.0)),
        Unit::Tsp => Some((Dimension::Volume, 5.0)),
        Unit::Tbsp => Some((Dimension::Volume, 15.0)),
        Unit::Cup => Some((Dimension::Volume, 250.0)),
        Unit::Unit | Unit::Pinch => None,
    }
}

pub fn dimension_of(unit: Option<Unit>) -> Dimension {
    match unit {
        None => Dimension::None,
        Some(Unit::Unit) => Dimension::Count,
        Some(Unit::Pinch) => Dimension::Pinch,
        Some(other) => to_base_factor(other).map(|(d, _)| d).unwrap_or(Dimension::None),
    }
}

/// Convertit vers l'unité de base de sa dimension (g, ml) ; les dimensions sans
/// conversion (count, pinch, none) passent telles quelles.
pub fn to_base(quantity: f64, unit: Option<Unit>) -> f64 {
    match unit.and_then(to_base_factor) {
        Some((_, factor)) => quantity * factor,
        None => quantity,
    }
}

/// Unité de base d'une dimension (celle stockée en base de données).
pub fn base_unit_of(dimension: Dimension) -> Option<Unit> {
    match dimension {
        Dimension::Mass => Some(Unit::G),
        Dimension::Volume => Some(Unit::Ml),
        Dimension::Count => Some(Unit::Unit),
        Dimension::Pinch => Some(Unit::Pinch),
        Dimension::None => None,
    }
}

/// Arrondi au centième (équivalent de `Math.round(x * 100) / 100` pour x ≥ 0).
pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayQuantity {
    pub quantity: f64,
    pub unit: Option<Unit>,
}

/// Convertit une quantité en unité de base vers l'unité d'affichage la plus
/// lisible du système demandé.
pub fn display_quantity(base_quantity: f64, unit: Option<Unit>, system: UnitSystem) -> DisplayQuantity {
    let shown = |quantity: f64, unit: Unit| DisplayQuantity {
        quantity: round2(quantity),
        unit: Some(unit),
    };
    match dimension_of(unit) {
        Dimension::Mass => match system {
            UnitSystem::Imperial if base_quantity >= 453.592 => shown(base_quantity / 453.592, Unit::Lb),
            UnitSystem::Imperial => shown(base_quantity / 28.35, Unit::Oz),
            UnitSystem::Metric if base_quantity >= 1000.0 => shown(base_quantity / 1000.0, Unit::Kg),
            UnitSystem::Metric => shown(base_quantity, Unit::G),
        },
        Dimension::Volume => match system {
            UnitSystem::Imperial if base_quantity >= 60.0 => shown(base_quantity / 250.0, Unit::Cup),
            UnitSystem::Imperial if base_quantity >= 15.0 => shown(base_quantity / 15.0, Unit::Tbsp),
            UnitSystem::Imperial => shown(base_quantity / 5.0, Unit::Tsp),
            UnitSystem::Metric if base_quantity >= 1000.0 => shown(base_quantity / 1000.0, Unit::L),
            UnitSystem::Metric => shown(base_quantity, Unit::Ml),
        },
        _ => DisplayQuantity {
            quantity: round2(base_quantity),
            unit,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_vers_la_base() {
        assert_eq!(to_base(2.0, Some(Unit::Kg)), 2000.0);
        assert_eq!(to_base(1.0, Some(Unit::Cup)), 250.0);
        assert_eq!(to_base(3.0, Some(Unit::Unit)), 3.0);
        assert_eq!(to_base(3.0, None), 3.0);
    }

    #[test]
    fn dimensions() {
        assert_eq!(dimension_of(Some(Unit::Oz)), Dimension::Mass);
        assert_eq!(dimension_of(Some(Unit::Tsp)), Dimension::Volume);
        assert_eq!(dimension_of(Some(Unit::Unit)), Dimension::Count);
        assert_eq!(dimension_of(Some(Unit::Pinch)), Dimension::Pinch);
        assert_eq!(dimension_of(None), Dimension::None);
    }

    #[test]
    fn affichage_metrique() {
        assert_eq!(
            display_quantity(1500.0, Some(Unit::G), UnitSystem::Metric),
            DisplayQuantity {
                quantity: 1.5,
                unit: Some(Unit::Kg)
            }
        );
        assert_eq!(
            display_quantity(350.0, Some(Unit::G), UnitSystem::Metric),
            DisplayQuantity {
                quantity: 350.0,
                unit: Some(Unit::G)
            }
        );
        assert_eq!(
            display_quantity(1250.0, Some(Unit::Ml), UnitSystem::Metric),
            DisplayQuantity {
                quantity: 1.25,
                unit: Some(Unit::L)
            }
        );
    }

    #[test]
    fn affichage_imperial() {
        assert_eq!(
            display_quantity(907.184, Some(Unit::G), UnitSystem::Imperial),
            DisplayQuantity {
                quantity: 2.0,
                unit: Some(Unit::Lb)
            }
        );
        assert_eq!(
            display_quantity(56.7, Some(Unit::G), UnitSystem::Imperial),
            DisplayQuantity {
                quantity: 2.0,
                unit: Some(Unit::Oz)
            }
        );
        assert_eq!(
            display_quantity(500.0, Some(Unit::Ml), UnitSystem::Imperial),
            DisplayQuantity {
                quantity: 2.0,
                unit: Some(Unit::Cup)
            }
        );
        assert_eq!(
            display_quantity(30.0, Some(Unit::Ml), UnitSystem::Imperial),
            DisplayQuantity {
                quantity: 2.0,
                unit: Some(Unit::Tbsp)
            }
        );
        assert_eq!(
            display_quantity(10.0, Some(Unit::Ml), UnitSystem::Imperial),
            DisplayQuantity {
                quantity: 2.0,
                unit: Some(Unit::Tsp)
            }
        );
    }

    #[test]
    fn sans_conversion() {
        assert_eq!(
            display_quantity(3.333, Some(Unit::Unit), UnitSystem::Imperial),
            DisplayQuantity {
                quantity: 3.33,
                unit: Some(Unit::Unit)
            }
        );
        assert_eq!(
            display_quantity(2.0, None, UnitSystem::Metric),
            DisplayQuantity {
                quantity: 2.0,
                unit: None
            }
        );
    }
}

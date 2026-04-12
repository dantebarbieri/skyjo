use skyjo_core::rules::{Rules, StandardRules};

#[test]
fn penalty_solo_lowest_no_penalty() {
    let rules = StandardRules;
    // Goer has 5, others all higher -> solo lowest, no penalty
    assert_eq!(rules.apply_going_out_penalty(5, 10, true), 5);
}

#[test]
fn penalty_not_lowest_doubles() {
    let rules = StandardRules;
    // Goer has 15, someone has 10 -> not solo lowest, doubled
    assert_eq!(rules.apply_going_out_penalty(15, 10, false), 30);
}

#[test]
fn penalty_tied_lowest_doubles() {
    let rules = StandardRules;
    // Goer has 10, tied with another player (not solo lowest)
    assert_eq!(rules.apply_going_out_penalty(10, 10, false), 20);
}

#[test]
fn penalty_negative_score_no_penalty() {
    let rules = StandardRules;
    // Goer has -3, not solo lowest -> no penalty because score <= 0
    assert_eq!(rules.apply_going_out_penalty(-3, -5, false), -3);
}

#[test]
fn penalty_zero_score_no_penalty() {
    let rules = StandardRules;
    // Goer has 0, not solo lowest -> no penalty because score <= 0
    assert_eq!(rules.apply_going_out_penalty(0, -1, false), 0);
}

#[test]
fn penalty_negative_and_solo_lowest() {
    let rules = StandardRules;
    // Goer has -5, solo lowest -> no penalty
    assert_eq!(rules.apply_going_out_penalty(-5, 10, true), -5);
}

pub mod command;

#[cfg(test)]
mod tests {
    use super::command::{
        calculate_level, calculate_xp, total_xp_to_reach_level, xp_required_for_level, JobType,
    };

    #[test]
    fn xp_calculation_normal_job() {
        let xp = calculate_xp(100, JobType::Normal, false);
        assert_eq!(xp, 100);
    }

    #[test]
    fn xp_calculation_special_job() {
        let xp = calculate_xp(100, JobType::Special, false);
        assert_eq!(xp, 300);
    }

    #[test]
    fn xp_calculation_htc_job() {
        let xp = calculate_xp(100, JobType::Htc, false);
        assert_eq!(xp, 200);
    }

    #[test]
    fn xp_calculation_reverse_bonus() {
        let xp = calculate_xp(100, JobType::Normal, true);
        assert_eq!(xp, 150);
    }

    #[test]
    fn level_progression_boundaries() {
        assert_eq!(total_xp_to_reach_level(1), 0);
        assert_eq!(total_xp_to_reach_level(2), 1500);
        assert_eq!(xp_required_for_level(1), 1500);
        assert_eq!(xp_required_for_level(9), 5500);
        assert_eq!(xp_required_for_level(10), 2500);
        assert_eq!(xp_required_for_level(50), 7500);
    }

    #[test]
    fn transition_level_9_to_10() {
        let before = total_xp_to_reach_level(10) - 1;
        let after = total_xp_to_reach_level(10);
        let progress_before = calculate_level(before);
        let progress_after = calculate_level(after);
        assert_eq!(progress_before.level, 9);
        assert_eq!(progress_after.level, 10);
        assert_eq!(progress_after.xp_into_level, 0);
    }

    #[test]
    fn transition_level_49_to_50() {
        let before = total_xp_to_reach_level(50) - 1;
        let after = total_xp_to_reach_level(50);
        let progress_before = calculate_level(before);
        let progress_after = calculate_level(after);
        assert_eq!(progress_before.level, 49);
        assert_eq!(progress_after.level, 50);
        assert_eq!(progress_after.xp_into_level, 0);
    }

    #[test]
    fn transition_level_99_to_100() {
        let before = total_xp_to_reach_level(100) - 1;
        let after = total_xp_to_reach_level(100);
        let progress_before = calculate_level(before);
        let progress_after = calculate_level(after);
        assert_eq!(progress_before.level, 99);
        assert_eq!(progress_after.level, 100);
        assert_eq!(progress_after.xp_into_level, 0);
        assert_eq!(progress_after.xp_needed_for_next_level, 0);
    }
}

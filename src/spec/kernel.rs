use crate::spec::case::SemanticsProfile;

impl SemanticsProfile {
    pub fn allows_local_reasoning_after_contradiction(&self) -> bool {
        matches!(self, Self::Paraconsistent)
    }

    pub fn quarantines_query_touching_contradiction(&self) -> bool {
        matches!(self, Self::Classical)
    }
}

pub(crate) mod view_owner;

pub(crate) use view_owner::{
    build_auto_revision_draft_payload, build_candidate_draft_payload,
    build_chapter_draft_analysis_view_fragments, ChapterDraftAnalysisViewFragments,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use view_owner::build_chapter_draft_view_payload_owner_contract;

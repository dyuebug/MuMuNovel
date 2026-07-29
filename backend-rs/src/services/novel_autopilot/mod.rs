pub(crate) mod book_polish_adapter;
pub(crate) mod book_polish_repository;

#[cfg(test)]
mod book_polish_tests;
pub(crate) mod book_review_adapter;
pub(crate) mod book_review_repository;
pub(crate) mod book_review_service;
mod budget_guard;

#[cfg(test)]
mod book_review_tests;
pub(crate) mod career_adapter;
pub(crate) mod chapter_adapter;
pub(crate) mod chapter_analysis_adapter;
pub(crate) mod chapter_analysis_repository;
pub(crate) mod chapter_repair_adapter;
pub(crate) mod chapter_repair_repository;
pub(crate) mod chapter_repository;

#[cfg(test)]
mod chapter_repository_tests;
pub(crate) mod character_adapter;
pub(crate) mod character_repository;
pub(crate) mod completion_gate_service;

#[cfg(test)]
mod completion_gate_tests;
pub(crate) mod coordinator;
pub(crate) mod export_adapter;
pub(crate) mod export_repository;

#[cfg(test)]
mod export_tests;
pub(crate) mod facts;
pub(crate) mod foundation_adapter;
pub(crate) mod organization_adapter;
pub(crate) mod outline_adapter;
pub(crate) mod outline_expansion_adapter;
pub(crate) mod outline_expansion_repository;
pub(crate) mod outline_repository;
pub(crate) mod output_observer;
pub(crate) mod repository;
pub(crate) mod router;
pub(crate) mod types;
pub(crate) mod world_adapter;

#[cfg(test)]
mod character_repository_tests;

#[cfg(test)]
mod outline_expansion_repository_tests;

#[cfg(test)]
mod outline_repository_tests;

#[cfg(test)]
mod tests;

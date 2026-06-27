from migrator_app.models import Base, load_all_models


def test_load_all_models_registers_alembic_metadata_tables():
    loaded_model_names = load_all_models()

    assert "Project" in loaded_model_names
    assert "Chapter" in loaded_model_names
    assert "BatchGenerationSnapshot" in loaded_model_names
    assert "projects" in Base.metadata.tables
    assert "chapters" in Base.metadata.tables
    assert "batch_generation_snapshots" in Base.metadata.tables

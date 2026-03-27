"""新增批量生成运行时存储表

Revision ID: 20260325_batch_runtime_store
Revises: 20260323_proj_quality_prefs
Create Date: 2026-03-25 09:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260325_batch_runtime_store'
down_revision: Union[str, None] = '20260323_proj_quality_prefs'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'chapter_draft_attempts',
        sa.Column('id', sa.String(length=36), nullable=False),
        sa.Column('project_id', sa.String(length=36), nullable=False),
        sa.Column('chapter_id', sa.String(length=36), nullable=True),
        sa.Column('batch_task_id', sa.String(length=36), nullable=True),
        sa.Column('source', sa.String(length=40), nullable=False, server_default='chapter', comment='来源：chapter/batch/regenerate'),
        sa.Column('attempt_state', sa.String(length=40), nullable=False, server_default='candidate', comment='尝试状态'),
        sa.Column('quality_gate_action', sa.String(length=40), nullable=True, comment='质量门动作'),
        sa.Column('quality_gate_decision', sa.String(length=40), nullable=True, comment='质量门决策'),
        sa.Column('word_count', sa.Integer(), nullable=False, server_default='0', comment='草稿字数'),
        sa.Column('summary_preview', sa.Text(), nullable=True, comment='摘要预览'),
        sa.Column('content_preview', sa.Text(), nullable=True, comment='内容预览'),
        sa.Column('quality_metrics', sa.JSON(), nullable=True, comment='质量指标'),
        sa.Column('repair_payload', sa.JSON(), nullable=True, comment='修复载荷'),
        sa.Column('created_at', sa.DateTime(), server_default=sa.text('CURRENT_TIMESTAMP'), nullable=True, comment='创建时间'),
        sa.ForeignKeyConstraint(['batch_task_id'], ['batch_generation_tasks.id'], ondelete='SET NULL'),
        sa.ForeignKeyConstraint(['chapter_id'], ['chapters.id'], ondelete='SET NULL'),
        sa.ForeignKeyConstraint(['project_id'], ['projects.id'], ondelete='CASCADE'),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_table(
        'batch_generation_snapshots',
        sa.Column('id', sa.String(length=36), nullable=False),
        sa.Column('batch_task_id', sa.String(length=36), nullable=False),
        sa.Column('latest_quality_metrics', sa.JSON(), nullable=True, comment='最新质量指标'),
        sa.Column('quality_metrics_history', sa.JSON(), nullable=True, comment='质量指标历史'),
        sa.Column('quality_metrics_summary', sa.JSON(), nullable=True, comment='质量指标汇总'),
        sa.Column('created_at', sa.DateTime(), server_default=sa.text('CURRENT_TIMESTAMP'), nullable=True, comment='创建时间'),
        sa.Column('updated_at', sa.DateTime(), server_default=sa.text('CURRENT_TIMESTAMP'), nullable=True, comment='更新时间'),
        sa.ForeignKeyConstraint(['batch_task_id'], ['batch_generation_tasks.id'], ondelete='CASCADE'),
        sa.PrimaryKeyConstraint('id'),
        sa.UniqueConstraint('batch_task_id'),
    )
    op.create_index('ix_chapter_draft_attempts_project_id', 'chapter_draft_attempts', ['project_id'])
    op.create_index('ix_chapter_draft_attempts_chapter_id', 'chapter_draft_attempts', ['chapter_id'])
    op.create_index('ix_chapter_draft_attempts_batch_task_id', 'chapter_draft_attempts', ['batch_task_id'])


def downgrade() -> None:
    op.drop_index('ix_chapter_draft_attempts_batch_task_id', table_name='chapter_draft_attempts')
    op.drop_index('ix_chapter_draft_attempts_chapter_id', table_name='chapter_draft_attempts')
    op.drop_index('ix_chapter_draft_attempts_project_id', table_name='chapter_draft_attempts')
    op.drop_table('batch_generation_snapshots')
    op.drop_table('chapter_draft_attempts')

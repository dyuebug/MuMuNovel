"""Store password verifiers as unbounded text.

Revision ID: 20260712_password_hash_phc_text
Revises: 20260517_project_core_defaults
Create Date: 2026-07-12 12:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = "20260712_password_hash_phc_text"
down_revision: Union[str, None] = "20260517_project_core_defaults"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


PASSWORD_HASH_COMMENT = "密码校验值（Argon2 PHC 或兼容的 legacy SHA256）"
LEGACY_PASSWORD_HASH_COMMENT = "密码哈希（SHA256）"


def upgrade() -> None:
    op.alter_column(
        "user_passwords",
        "password_hash",
        existing_type=sa.String(length=64),
        type_=sa.Text(),
        existing_nullable=False,
    )
    op.execute(
        "COMMENT ON COLUMN user_passwords.password_hash "
        f"IS '{PASSWORD_HASH_COMMENT}'"
    )


def downgrade() -> None:
    op.execute(
        """
        DO $$
        BEGIN
            IF EXISTS (
                SELECT 1
                FROM user_passwords
                WHERE length(password_hash) > 64
            ) THEN
                RAISE EXCEPTION
                    'cannot downgrade password_hash to VARCHAR(64): long verifier exists';
            END IF;
        END
        $$
        """
    )
    op.alter_column(
        "user_passwords",
        "password_hash",
        existing_type=sa.Text(),
        type_=sa.String(length=64),
        existing_nullable=False,
    )
    op.execute(
        "COMMENT ON COLUMN user_passwords.password_hash "
        f"IS '{LEGACY_PASSWORD_HASH_COMMENT}'"
    )

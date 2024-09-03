use crate::m20240828_090802_role::Role;
use crate::m20240828_090938_user::User;
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserRole::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserRole::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserRole::UserId).integer().not_null())
                    .col(ColumnDef::new(UserRole::RoleId).integer().not_null())
                    .to_owned(),
            )
            .await?;

        let user_foreign_key = ForeignKey::create()
            .name("FK_user_role_user_id")
            .from(UserRole::Table, UserRole::UserId)
            .to(User::Table, User::Id)
            .on_delete(ForeignKeyAction::Cascade)
            .on_update(ForeignKeyAction::Cascade)
            .to_owned();

        let role_foreign_key = ForeignKey::create()
            .name("FK_user_role_role_id")
            .from(UserRole::Table, UserRole::RoleId)
            .to(Role::Table, Role::Id)
            .on_delete(ForeignKeyAction::Cascade)
            .on_update(ForeignKeyAction::Cascade)
            .to_owned();

        manager.create_foreign_key(user_foreign_key).await?;
        manager.create_foreign_key(role_foreign_key).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserRole::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserRole {
    Table,
    Id,
    UserId,
    RoleId,
}

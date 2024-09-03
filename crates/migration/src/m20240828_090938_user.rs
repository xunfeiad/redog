use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(User::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(User::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(User::Password).text().not_null())
                    .col(ColumnDef::new(User::NickName).string().null())
                    .col(ColumnDef::new(User::Avatar).text().not_null())
                    .col(
                        ColumnDef::new(User::Mobile)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(User::Email).string().not_null().unique_key())
                    .col(
                        ColumnDef::new(User::CreateTime)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(User::UpdateTime)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(User::Status).small_unsigned().default(1u8))
                    .col(
                        ColumnDef::new(User::LastLoginTime)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(User::Deleted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        let username_index = Index::create()
            .if_not_exists()
            .name("idx-user-username")
            .table(User::Table)
            .col(User::Username)
            .to_owned();

        let mobile_index = Index::create()
            .if_not_exists()
            .name("idx-mobile-username")
            .table(User::Table)
            .col(User::Username)
            .to_owned();

        let email_index = Index::create()
            .if_not_exists()
            .name("idx-email-username")
            .table(User::Table)
            .col(User::Username)
            .to_owned();
        manager.create_index(username_index).await?;
        manager.create_index(mobile_index).await?;
        manager.create_index(email_index).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub(crate) enum User {
    Table,
    Id,
    Username,
    Password,
    NickName,
    Avatar,
    Mobile,
    Email,
    CreateTime,
    UpdateTime,
    Status,
    LastLoginTime,
    Deleted,
}

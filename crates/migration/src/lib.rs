pub use sea_orm_migration::prelude::*;
mod m20240828_090802_role;
mod m20240828_090938_user;
mod m20240828_093850_user_role;
mod m20240902_090827_message;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240828_090802_role::Migration),
            Box::new(m20240828_090938_user::Migration),
            Box::new(m20240828_093850_user_role::Migration),
            Box::new(m20240902_090827_message::Migration),
        ]
    }
}

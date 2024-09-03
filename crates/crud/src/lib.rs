pub mod auth;
use model::user::ActiveModel;
use sea_orm::*;
use std::error::Error;
use std::marker::PhantomData;

pub trait HasId: ActiveModelTrait {
    fn set_id(&mut self, v: i32);
}
/// T: EntityTrait
/// M: ModelTrait
/// C: ColumnTrait
/// Db: ConnectionTrait
pub struct Crud<'a, T, M, C, Db> {
    phantom: PhantomData<T>,
    phantom_model: PhantomData<M>,
    phantom_column: PhantomData<C>,
    phantom_lifetime: PhantomData<&'a ()>,
    phantom_db: PhantomData<Db>,
}

impl<'a, T, M, C, Db> Crud<'a, T, M, C, Db>
where
    M: ModelTrait,
    T: EntityTrait<Model = M, Column = C>,
    T::PrimaryKey: PrimaryKeyTrait,
    <T::PrimaryKey as PrimaryKeyTrait>::ValueType: From<i32>,
    C: ColumnTrait,
    Db: ConnectionTrait,
    Select<T>: PaginatorTrait<'a, Db>,
    <Select<T> as PaginatorTrait<'a, Db>>::Selector: SelectorTrait<Item = M>,
{
    pub fn new() -> Self {
        Crud {
            phantom: PhantomData,
            phantom_model: PhantomData,
            phantom_column: PhantomData,
            phantom_lifetime: PhantomData,
            phantom_db: PhantomData,
        }
    }
    pub async fn get(&self, db: &DbConn, id: i32) -> Result<Option<M>, DbErr> {
        T::find_by_id(id).one(db).await
    }

    /// If ok, returns (post models, num pages).
    pub async fn get_multi(
        &self,
        db: &'a Db,
        page: u64,
        page_size: u64,
        order_column: C,
    ) -> Result<(Vec<M>, u64), DbErr> {
        // Setup paginator
        let paginator = T::find().order_by_asc(order_column).paginate(db, page_size);
        let num_pages = paginator.num_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn create<'b, A>(&self, db: &DbConn, active_model: A) -> Result<A, DbErr>
    where
        A: ActiveModelTrait + Send + Sync + ActiveModelBehavior + 'b,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        active_model.save(db).await
    }

    pub async fn update_by_id<'c, A>(
        &self,
        db: &DbConn,
        id: i32,
        mut active_model: A,
    ) -> Result<M, DbErr>
    where
        A: From<M> + HasId + ActiveModelTrait + Send + Sync + ActiveModelBehavior + 'c,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
        <A as ActiveModelTrait>::Entity: EntityTrait<Model = M>,
    {
        let _: A = T::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::Custom("No data queried.".to_owned()))
            .map(Into::into)?;

        active_model.set_id(id);
        active_model.update(db).await
    }

    pub async fn delete<'d, A>(&self, db: &DbConn, id: i32) -> Result<DeleteResult, DbErr>
    where
        A: From<M> + HasId + ActiveModelTrait + Send + Sync + ActiveModelBehavior + 'd,
    {
        let active_model: A = T::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::Custom("No data queried.".to_owned()))
            .map(Into::into)?;

        active_model.delete(db).await
    }
}

impl HasId for ActiveModel {
    fn set_id(&mut self, v: i32) {
        self.id = Set(v)
    }
}

// for `test`
pub async fn get_connection() -> Result<DatabaseConnection, Box<dyn Error>> {
    let conn = Database::connect("postgres://xunfei:xunfei123@localhost:5432/rtc").await?;
    Ok(conn)
}

#[allow(unused_imports)]
mod test {

    use crate::get_connection;
    use crate::Crud;
    use chrono::*;
    use model::user::{ActiveModel, Column, Entity, Model};
    use sea_orm::*;
    use std::{default, error::Error, time::Duration};

    #[tokio::test]
    pub async fn test_get() -> Result<(), Box<dyn Error>> {
        let conn = get_connection().await.unwrap();
        let crud = Crud::<Entity, Model, Column, DatabaseConnection>::new();
        let a = crud.get(&conn, 2).await.unwrap();
        a.unwrap();
        Ok(())
    }

    #[tokio::test]
    pub async fn test_get_multi() -> Result<(), Box<dyn Error>> {
        let conn = get_connection().await.unwrap();
        let crud = Crud::<Entity, Model, Column, DatabaseConnection>::new();
        let a = crud.get_multi(&conn, 1, 10, Column::Id).await.unwrap();
        let (_, page_num) = a;
        assert!(page_num != 0);
        Ok(())
    }

    #[tokio::test]
    pub async fn test_update_by_id() -> Result<(), Box<dyn Error>> {
        let conn = get_connection().await.unwrap();
        let crud = Crud::<Entity, Model, Column, DatabaseConnection>::new();
        let active_model = ActiveModel {
            id: ActiveValue::NotSet,
            username: Set("xunfei1".to_string()),
            password: ActiveValue::NotSet,
            nick_name: ActiveValue::NotSet,
            avatar: ActiveValue::NotSet,
            mobile: ActiveValue::NotSet,
            email: ActiveValue::NotSet,
            create_time: ActiveValue::NotSet,
            update_time: ActiveValue::NotSet,
            status: ActiveValue::NotSet,
            last_login_time: ActiveValue::NotSet,
            deleted: ActiveValue::NotSet,
        };
        let model = crud.update_by_id(&conn, 2, active_model).await?;

        assert_eq!(model.username, "xunfei1".to_owned());
        Ok(())
    }

    #[tokio::test]
    pub async fn test_create() -> Result<(), Box<dyn Error>> {
        let conn = get_connection().await.unwrap();
        let crud = Crud::<Entity, Model, Column, DatabaseConnection>::new();
        let offset = FixedOffset::east_opt(5 * 60 * 60).unwrap();
        let now_with_offset = Utc::now().with_timezone(&offset);
        let active_model = ActiveModel {
            id: ActiveValue::NotSet,
            username: Set("erpang1".to_string()),
            password: Set("gougou1".to_string()),
            nick_name: Set(Some("gougou1".to_string())),
            avatar: Set("gougou1".to_string()),
            mobile: Set("gougou1".to_string()),
            email: Set("gougou1".to_string()),
            create_time: Set(Some(now_with_offset)),
            update_time: Set(Some(now_with_offset)),
            status: Set(Some(1)),
            last_login_time: Set(Some(now_with_offset)),
            deleted: Set(false),
        };
        crud.create(&conn, active_model).await?;
        Ok(())
    }

    #[tokio::test]
    pub async fn test_delete() -> Result<(), Box<dyn Error>> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let conn = get_connection().await.unwrap();
        let crud = Crud::<Entity, Model, Column, DatabaseConnection>::new();

        crud.delete::<ActiveModel>(&conn, 2).await?;
        Ok(())
    }
}

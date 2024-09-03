use error::RtcResult;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

// #[proc_macro_derive(Crud)]
// pub fn derive_crud_trait(input: TokenStream) -> TokenStream {
//     let input = parse_macro_input!(input as DeriveInput);
//     let name = &input.ident;
//     let expanded = quote! {
//             impl Crud<#name> for #name{
//                   async fn get(&self, pool: &PgPool, id: u64) -> RtcResult<#name> {
//         let res = sqlx::query_as!(User, r#"Select * from {#name} where id=$1"#, id).fetch_all(&pool).await?;
//         Ok(res)
//     }
//
//                 async fn get_multi(pool: &pool::PgPool, limit: u64, offset: u64) -> RtcResult<Vec<#name>> {
//                     todo!()
//                 }
//
//                 async fn count(pool: &pool::PgPool) -> error::RtcResult<u64> {
//                     todo!()
//                 }
//
//                 async fn create(pool: &pool::PgPool, schema: #name) -> RtcResult<u64> {
//                     todo!()
//                 }
//
//                 async fn update(pool: &pool::PgPool, update: #name, create: #name) -> RtcResult<#name> {
//                     todo!()
//                 }
//
//                 async fn delete(pool: &pool::PgPool, id: u64) -> RtcResult<()> {
//                     todo!()
//                 }
//     }
//         };
//     expanded.into()
// }

//! Read-only old-column bags and receipt evidence for explicitly owned archive clones.
fn archive_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
pub fn write_archive_evidence(path: &std::path::Path, value: &serde_json::Value) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(value).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}
pub async fn archive_snapshot(
    pool: &sqlx::PgPool,
    columns: &std::collections::BTreeMap<String, Vec<String>>,
    maximum_original_migration: i32,
) -> serde_json::Value {
    use serde_json::{Value, json};
    use sqlx::Row;
    let mut tables = serde_json::Map::new();
    let mut column_definitions = serde_json::Map::new();
    for (table, names) in columns {
        let projection = names
            .iter()
            .map(|v| format!("t.{}", archive_ident(v)))
            .collect::<Vec<_>>()
            .join(",");
        let migration_predicate = format!(" WHERE version<={maximum_original_migration}");
        let predicate = match table.as_str() {
            "meta" => " WHERE key<>'state_format_version'",
            "schema_migrations" => migration_predicate.as_str(),
            _ => "",
        };
        let query = format!(
            "SELECT jsonb_build_object('count',count(*),'sha256',encode(sha256(convert_to(COALESCE(string_agg(h,'' ORDER BY h),''),'UTF8')),'hex')) FROM (SELECT encode(sha256(convert_to(to_jsonb(t)::text,'UTF8')),'hex') h FROM (SELECT {projection} FROM public.{} t{predicate}) t) bag",
            archive_ident(table)
        );
        let bag: Value = sqlx::query_scalar(&query).fetch_one(pool).await.unwrap();
        tables.insert(table.clone(), bag);
        let defs:Value=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_object('name',a.attname,'type',format_type(a.atttypid,a.atttypmod),'notnull',a.attnotnull,'identity',a.attidentity,'generated',a.attgenerated,'default',pg_get_expr(d.adbin,d.adrelid)) ORDER BY a.attnum) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum WHERE n.nspname='public' AND c.relname=$1 AND a.attname=ANY($2) AND NOT a.attisdropped")
            .bind(table).bind(names).fetch_one(pool).await.unwrap();
        column_definitions.insert(table.clone(), defs);
    }
    let sequence_names:Vec<String>=sqlx::query_scalar("SELECT c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind='S' ORDER BY c.relname").fetch_all(pool).await.unwrap();
    let mut sequences = serde_json::Map::new();
    for name in sequence_names {
        let row = sqlx::query(&format!(
            "SELECT last_value,is_called FROM public.{}",
            archive_ident(&name)
        ))
        .fetch_one(pool)
        .await
        .unwrap();
        sequences.insert(name,json!({"last_value":row.get::<i64,_>("last_value"),"is_called":row.get::<bool,_>("is_called")}));
    }
    json!({"old_table_column_bags":tables,"old_column_definitions":column_definitions,"sequence_states":sequences})
}

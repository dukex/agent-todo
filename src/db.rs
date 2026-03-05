use worker::Env;

/// Get the D1 database binding from the environment.
pub fn get_db(env: &Env) -> Result<worker::D1Database, worker::Error> {
    env.d1("DB")
}

/// Get the KV namespace binding from the environment.
pub fn get_kv(env: &Env) -> Result<worker::kv::KvStore, worker::Error> {
    env.kv("KV")
}

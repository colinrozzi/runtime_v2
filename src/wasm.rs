use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;
use wasmtime::component::{Component, Instance, Linker};
use wasmtime::{Engine, Store};

use crate::config::ActorConfig;

pub struct WasmActor {
    engine: Engine,
    component: Component,
    linker: Linker<()>,
    supports_http: bool,
}

impl WasmActor {
    pub fn from_file<P: AsRef<Path>>(manifest_path: P) -> Result<Self> {
        let config = ActorConfig::from_file(manifest_path)?;
        
        let engine = Engine::default();
        let wasm_bytes = std::fs::read(&config.component_path)
            .context("Failed to read WASM file")?;
        let component = Component::new(&engine, &wasm_bytes)
            .context("Failed to create WASM component")?;
        
        let mut linker = Linker::new(&engine);
        
        // Set up basic actor host functions
        let mut runtime = linker.instance("ntwk:simple-actor/runtime")
            .context("Failed to create runtime instance")?;
        
        runtime.func_wrap("log", |_, (msg,): (String,)| {
            println!("[WASM] {}", msg);
            Ok(())
        })?;
        
        runtime.func_wrap("send", |_, (actor_id, msg): (String, Vec<u8>)| {
            println!("Message send requested to {}", actor_id);
            Ok(())
        })?;

        Ok(WasmActor {
            engine,
            component,
            linker,
            supports_http: config.supports_http(),
        })
    }

    fn call_instance_func<T, U>(&self, store: &mut Store<()>, instance: &Instance, name: &str, args: T) -> Result<U>
    where
        T: wasmtime::component::Lower,
        U: wasmtime::component::Lift,
    {
        let (_, instance_idx) = self.component.export_index(None, "ntwk:simple-actor/actor")?;
        let (_, func_idx) = self.component.export_index(Some(&instance_idx), name)?;
        
        let func = instance.get_func(&mut *store, func_idx)
            .context(format!("Failed to get function {}", name))?;
        
        let typed = func.typed::<T, U>(&mut *store)?;
        Ok(typed.call(&mut *store, args)?)
    }

    fn call_http_func<T, U>(&self, store: &mut Store<()>, instance: &Instance, name: &str, args: T) -> Result<U>
    where
        T: wasmtime::component::Lower,
        U: wasmtime::component::Lift,
    {
        let (_, instance_idx) = self.component.export_index(None, "ntwk:simple-http-actor/http-actor")?;
        let (_, func_idx) = self.component.export_index(Some(&instance_idx), name)?;
        
        let func = instance.get_func(&mut *store, func_idx)
            .context(format!("Failed to get function {}", name))?;
        
        let typed = func.typed::<T, U>(&mut *store)?;
        Ok(typed.call(&mut *store, args)?)
    }

    pub fn init(&self) -> Result<Value> {
        let mut store = Store::new(&self.engine, ());
        let instance = self.linker.instantiate(&mut store, &self.component)?;

        let (result,) = self.call_instance_func::<(), (Vec<u8>,)>(&mut store, &instance, "init", ())?;
        Ok(serde_json::from_slice(&result)?)
    }

    pub fn handle_message(&self, msg: &Value, state: &Value) -> Result<(Value, Value)> {
        let mut store = Store::new(&self.engine, ());
        let instance = self.linker.instantiate(&mut store, &self.component)?;

        let msg_bytes = serde_json::to_vec(msg)?;
        let state_bytes = serde_json::to_vec(state)?;

        let (result,) = self.call_instance_func::<(Vec<u8>, Vec<u8>), (Vec<u8>,)>(
            &mut store, 
            &instance,
            "handle",
            (msg_bytes, state_bytes)
        )?;

        let new_state: Value = serde_json::from_slice(&result)?;
        Ok((msg.clone(), new_state))
    }

    pub fn handle_http(
        &self,
        method: String,
        uri: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        state: &Value,
    ) -> Result<(u16, Vec<(String, String)>, Option<Vec<u8>>, Value)> {
        if !self.supports_http {
            anyhow::bail!("Actor does not support HTTP");
        }

        let mut store = Store::new(&self.engine, ());
        let instance = self.linker.instantiate(&mut store, &self.component)?;

        let request = serde_json::json!({
            "method": method,
            "uri": uri,
            "headers": { "fields": headers },
            "body": body,
        });

        let request_bytes = serde_json::to_vec(&request)?;
        let state_bytes = serde_json::to_vec(state)?;

        let (result,) = self.call_http_func::<(Vec<u8>, Vec<u8>), (Vec<u8>,)>(
            &mut store,
            &instance,
            "handle-http",
            (request_bytes, state_bytes),
        )?;

        let response: Value = serde_json::from_slice(&result)?;
        let new_state = response["state"].clone();
        let http_response = response["response"].clone();

        let status = http_response["status"].as_u64().unwrap_or(500) as u16;
        let headers = http_response["headers"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let pair = v.as_array()?;
                        Some((pair[0].as_str()?.to_string(), pair[1].as_str()?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let body = http_response["body"]
            .as_array()
            .map(|arr| arr.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect());

        Ok((status, headers, body, new_state))
    }
}
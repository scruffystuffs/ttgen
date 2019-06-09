use std::fs::{self, File};
use std::io::{copy, prelude::*, stdout};
use std::path::Path;
use std::result::Result as StdResult;

use chrono::Utc;
use handlebars::{Context, Handlebars, Helper, Output, RenderContext, RenderError};
use once_cell::sync::Lazy;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::*;
use crate::spec::TemplateSpec;

static NAME: Lazy<String> = Lazy::new(|| clap::crate_name!().to_string());
static VERSION: Lazy<String> = Lazy::new(|| clap::crate_version!().to_string());
static DATESTAMP: Lazy<String> = Lazy::new(|| Utc::now().to_rfc3339());

fn pyprint(
    h: &Helper,
    _hb: &Handlebars,
    _c: &Context,
    _rc: &mut RenderContext,
    out: &mut Output,
) -> StdResult<(), RenderError> {
    let value = h
        .param(0)
        .ok_or_else(|| RenderError::new("pyprint helper missing first argument"))?
        .value();
    let value_if_null = h
        .param(1)
        .ok_or_else(|| RenderError::new("pyprint helper missing second argument"))?
        .value()
        .as_str()
        .ok_or_else(|| RenderError::new("pyprint second argument is not string"))?;
    let output = match value {
        Value::Bool(b) => match b {
            true => "True".to_string(),
            false => "False".to_string(),
        },
        Value::Null => value_if_null.to_string(),
        _ => value.to_string(),
    };
    out.write(&output)?;
    Ok(())
}

pub fn get_renderer() -> Handlebars {
    let mut hb = Handlebars::new();
    // hb.set_strict_mode(false);
    hb.register_template_string("rst_stamp", include_str!("builtins/rst_stamp.hbs"))
        .expect("rst stamp failed to compile");
    hb.register_helper("pyprint", Box::new(pyprint));
    hb
}

fn hash_file<P: AsRef<Path>>(p: P) -> Result<String> {
    let mut stream = File::open(p)?;
    let mut hasher = Sha256::new();
    copy(&mut stream, &mut hasher)?;
    Ok(format!("{:x}", hasher.result()))
}

fn create_root_map(spec: &TemplateSpec) -> Result<Map<String, Value>> {
    let mut root_map = Map::new();
    root_map.insert("name".to_string(), Value::from(&**NAME));
    root_map.insert("version".to_string(), Value::from(&**VERSION));
    root_map.insert("date".to_string(), Value::from(&**DATESTAMP));
    root_map.insert(
        "data_file".to_string(),
        Value::from(spec.data.display().to_string()),
    );
    root_map.insert(
        "template_file".to_string(),
        Value::from(spec.template.display().to_string()),
    );
    root_map.insert("data_hash".to_string(), Value::from(hash_file(&spec.data)?));
    root_map.insert(
        "template_hash".to_string(),
        Value::from(hash_file(&spec.template)?),
    );
    root_map.insert(
        "root".to_string(),
        serde_json::from_reader(File::open(&spec.data)?)?,
    );

    Ok(root_map)
}

pub fn render_with(spec: &TemplateSpec, hb: &Handlebars) -> Result<()> {
    let root_map = create_root_map(spec)?;
    let out_writer: Box<dyn Write> = match &spec.output {
        Some(output) => {
            if let Some(p) = &output.parent() {
                fs::create_dir_all(p)?;
            };
            Box::new(File::create(output)?)
        }
        None => Box::new(stdout()),
    };
    let mut tmpl_reader = File::open(&spec.template)?;
    hb.render_template_source_to_write(&mut tmpl_reader, &root_map, out_writer)?;
    Ok(())
}
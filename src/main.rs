extern crate env_logger;
extern crate rusoto_core;
extern crate rusoto_secretsmanager;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate uuid;

use std::collections::BTreeMap;
use std::env::args;
use std::error::Error;
use std::io::{self, Read};
use std::process;
use std::result::Result;

use envy;
use rusoto_core::region::Region;
use rusoto_secretsmanager::GetSecretValueRequest;
use rusoto_secretsmanager::PutSecretValueRequest;
use rusoto_secretsmanager::{SecretsManager, SecretsManagerClient};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
struct RegistryCredentials {
    #[serde(rename = "ServerURL")]
    server_url: String,
    #[serde(flatten)]
    registry_secret: RegistryCredentialsSecret,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct RegistryCredentialsSecret {
    #[serde(rename = "Username")]
    username: String,
    #[serde(rename = "Secret")]
    secret: String,
}

#[derive(Deserialize, Debug)]
struct SecretsManagerEnvConfig {
    docker_secretsmanager_name: String,
    docker_secretsmanager_key_arn: Option<String>,
}

struct SecretsManagerClientHelper {
    config: SecretsManagerEnvConfig,
    client: SecretsManagerClient,
}

impl SecretsManagerClientHelper {
    fn get_secret_map(&self) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
        let secret = self
            .client
            .get_secret_value(GetSecretValueRequest {
                secret_id: self.config.docker_secretsmanager_name.clone(),
                ..Default::default()
            })
            .sync()?;
        let secret_map = serde_json::from_str::<BTreeMap<String, String>>(
            secret
                .secret_string
                .ok_or("invalid secret format")?
                .as_ref(),
        )?;
        Ok(secret_map)
    }
    fn put_secret_map(&self, secret_map: BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
        let uuid_val = Uuid::new_v4()
            .to_hyphenated()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned();
        self.client
            .put_secret_value(PutSecretValueRequest {
                secret_id: self.config.docker_secretsmanager_name.clone(),
                secret_string: Some(serde_json::to_string(&secret_map)?),
                client_request_token: Some(uuid_val),
                ..Default::default()
            })
            .sync()?;
        Ok(())
    }
    fn remove_registry_credentials(&self, url: &str) -> Result<(), Box<dyn Error>> {
        let mut secret_map = self.get_secret_map()?;
        let _ = get_registry_credentials_secret(url, &secret_map)?;
        secret_map.remove(url);
        self.put_secret_map(secret_map)
    }
}

fn get_registry_credentials_secret(
    url: &str,
    secret_map: &BTreeMap<String, String>,
) -> Result<RegistryCredentialsSecret, Box<dyn Error>> {
    Ok(serde_json::from_str::<RegistryCredentialsSecret>(
        secret_map.get(url).ok_or("invalid url")?,
    )?)
}

fn run() -> Result<(), Box<dyn Error>> {
    let _ = env_logger::try_init();
    let client = SecretsManagerClientHelper {
        config: envy::from_env::<SecretsManagerEnvConfig>()?,
        client: SecretsManagerClient::new(Region::default()),
    };
    let command = args().nth(1).ok_or("invalid usage!")?;
    let mut buffer = String::new();
    match command.as_ref() {
        "erase" => {
            io::stdin().read_to_string(&mut buffer)?;
            client.remove_registry_credentials(&buffer.trim_end().to_string())?;
        }
        "get" => {
            io::stdin().read_to_string(&mut buffer)?;
            let url = buffer.trim_end().to_string();
            let secret_map = client.get_secret_map()?;
            let rcs = get_registry_credentials_secret(&url, &secret_map)?;
            println!(
                "{}",
                serde_json::to_string(&RegistryCredentials {
                    server_url: url,
                    registry_secret: rcs,
                })?
            );
        }
        "list" => {
            let secret_map = client.get_secret_map()?;
            let cred_map: BTreeMap<String, String> = secret_map
                .iter()
                .filter_map(|(k, v)| {
                    if let Ok(rcs) = serde_json::from_str::<RegistryCredentialsSecret>(v) {
                        return Some((k.to_owned(), rcs.username));
                    }
                    None
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&cred_map)?);
        }
        "store" => {
            io::stdin().read_to_string(&mut buffer)?;
            let creds: RegistryCredentials = serde_json::from_str(&buffer)?;
            let mut secret_map = client.get_secret_map()?;
            secret_map.insert(
                creds.server_url.clone(),
                serde_json::to_string(&creds.registry_secret)?,
            );
            client.put_secret_map(secret_map)?;
        }
        _ => process::exit(1),
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("error: {:+}", err);
        process::exit(1);
    }
}

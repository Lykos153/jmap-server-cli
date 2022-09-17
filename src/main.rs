/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Command Line Interface.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::{
    collections::HashMap,
    io::{BufRead, Write},
};

use clap::Parser;
use console::style;
use jmap_client::client::{Client, Credentials};
use modules::{
    account::cmd_account,
    cli::{Cli, Commands},
    domain::cmd_domain,
    get,
    group::cmd_group,
    import::cmd_import,
    list::cmd_list,
    post,
};

use crate::modules::OAuthResponse;

pub mod modules;

fn main() {
    let args = Cli::parse();
    let credentials = if let Some(credentials) = args.credentials {
        parse_credentials(&credentials)
    } else {
        let credentials =
            rpassword::prompt_password("\nEnter admin credentials or press [ENTER] to use OAuth: ")
                .unwrap();
        if !credentials.is_empty() {
            parse_credentials(&credentials)
        } else {
            oauth(&args.url)
        }
    };

    let client = Client::new()
        .credentials(credentials)
        .connect(&args.url)
        .unwrap_or_else(|err| {
            eprintln!("Failed to connect to JMAP server {}: {}.", args.url, err);
            std::process::exit(1);
        });

    match args.command {
        Commands::Account(command) => cmd_account(client, command),
        Commands::Domain(command) => cmd_domain(client, command),
        Commands::List(command) => cmd_list(client, command),
        Commands::Group(command) => cmd_group(client, command),
        Commands::Import(command) => cmd_import(client, command),
    }
}

fn parse_credentials(credentials: &str) -> Credentials {
    if let Some((account, secret)) = credentials.split_once(':') {
        Credentials::basic(account, secret)
    } else {
        Credentials::basic("admin", credentials)
    }
}

fn oauth(url: &str) -> Credentials {
    let metadata = get(&format!("{}/.well-known/oauth-authorization-server", url));
    let token_endpoint = metadata.property("token_endpoint");
    let mut params = HashMap::from_iter([("client_id".to_string(), "Stalwart_CLI".to_string())]);
    let response = post(metadata.property("device_authorization_endpoint"), &params);

    params.insert(
        "grant_type".to_string(),
        "urn:ietf:params:oauth:grant-type:device_code".to_string(),
    );
    params.insert(
        "device_code".to_string(),
        response.property("device_code").to_string(),
    );

    print!(
        "\nAuthenticate this request using code {} at {}. Please ENTER when done.",
        style(response.property("user_code")).bold(),
        style(response.property("verification_uri")).bold().dim()
    );

    std::io::stdout().flush().unwrap();
    std::io::stdin().lock().lines().next();

    let mut response = post(token_endpoint, &params);
    if let Some(serde_json::Value::String(access_token)) = response.remove("access_token") {
        Credentials::Bearer(access_token)
    } else {
        eprintln!(
            "OAuth failed with code {}.",
            response
                .get("error")
                .and_then(|s| s.as_str())
                .unwrap_or("<unknown>")
        );
        std::process::exit(1);
    }
}

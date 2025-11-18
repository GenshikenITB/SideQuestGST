use google_sheets4::{api::ValueRange, hyper, hyper_rustls, Sheets, chrono};
use serde_json::json;
use crate::models::{EventMessage, QuestPayload, RegistrationPayload, NewCommunityPayload, ProofPayload};

pub type HubType = Sheets<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>;

pub async fn process_event(hub: &HubType, spreadsheet_id: &str, event: EventMessage) {
    println!("Processing Event: {}", event.event_type);
    let now = chrono::Utc::now().to_rfc3339();

    match event.event_type.as_str() {
        "CREATE_QUEST" => {
            if let Ok(data) = serde_json::from_str::<QuestPayload>(&event.payload) {
                let values = vec![vec![
                    json!(data.quest_id),
                    json!(data.title),
                    json!(data.category),
                    json!(data.organizer_name),
                    json!(data.schedule),
                    json!(data.platform),
                    json!(data.description),
                    json!(data.deadline),
                    json!(now),
                ]];
                append_to_sheet(hub, spreadsheet_id, "Quests!A1", values).await;
            } else {
                eprintln!("Failed to parse CREATE_QUEST");
            }
        },
        
        "TAKE_QUEST" => {
            if let Ok(data) = serde_json::from_str::<RegistrationPayload>(&event.payload) {
                let values = vec![vec![
                    json!(data.quest_id),
                    json!(data.user_id),
                    json!(data.user_tag),
                    json!("ON_PROGRESS"),
                    json!(now),
                ]];
                append_to_sheet(hub, spreadsheet_id, "Participants!A1", values).await;
            }
        },

        "REGISTER_COMMUNITY" => {
            if let Ok(data) = serde_json::from_str::<NewCommunityPayload>(&event.payload) {
                let values = vec![vec![
                    json!(data.community_name),
                    json!(data.leader_id),
                    json!(now),
                ]];
                append_to_sheet(hub, spreadsheet_id, "Communities!A1", values).await;
            }
        },

        "SUBMIT_PROOF" => {
            if let Ok(data) = serde_json::from_str::<ProofPayload>(&event.payload) {
                let values = vec![vec![
                    json!(data.quest_id),
                    json!(data.user_id),
                    json!(data.proof_url),
                    json!(now),
                ]];
                append_to_sheet(hub, spreadsheet_id, "Submissions!A1", values).await;
            }
        },

        _ => println!("Unknown event type: {}", event.event_type),
    }
}

async fn append_to_sheet(hub: &HubType, spreadsheet_id: &str, range: &str, values: Vec<Vec<serde_json::Value>>) {
    let req = ValueRange { values: Some(values), ..Default::default() };
    let result = hub.spreadsheets().values_append(req, spreadsheet_id, range)
        .value_input_option("USER_ENTERED")
        .doit().await;

    match result {
        Ok(_) => println!("Success write to {}", range),
        Err(e) => eprintln!("Google Sheets API Error: {:?}", e),
    }
}
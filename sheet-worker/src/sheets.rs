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
                update_participant_status(hub, spreadsheet_id, &data.quest_id, &data.user_id, "COMPLETED").await;
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

async fn update_participant_status(hub: &HubType, spreadsheet_id: &str, quest_id: &str, user_id: &str, new_status: &str) {
    let read_result = hub.spreadsheets().values_get(spreadsheet_id, "Participants!A:D").doit().await;

    if let Ok((_, range)) = read_result {
        if let Some(rows) = range.values {
            for (index, row) in rows.iter().enumerate() {
                if row.len() >= 2 {
                    let q_id = row[0].as_str().unwrap_or("");
                    let u_id = row[1].as_str().unwrap_or("");

                    if q_id == quest_id && u_id == user_id {
                        let row_number = index + 1;
                        let update_range = format!("Participants!D{}", row_number);
                        
                        let req = ValueRange { 
                            values: Some(vec![vec![json!(new_status)]]), 
                            ..Default::default() 
                        };

                        let update = hub.spreadsheets().values_update(req, spreadsheet_id, &update_range)
                            .value_input_option("RAW")
                            .doit().await;
                            
                        match update {
                            Ok(_) => println!("✅ Updated User {} Quest {} to {}", u_id, q_id, new_status),
                            Err(e) => eprintln!("Failed to update status: {:?}", e),
                        }
                        return;
                    }
                }
            }
        }
    }
}

pub async fn check_deadlines_job(hub: &HubType, spreadsheet_id: &str) {
    println!("Running Deadline Check...");
    let quests_res = hub.spreadsheets().values_get(spreadsheet_id, "Quests!A:H").doit().await;

    let parts_res = hub.spreadsheets().values_get(spreadsheet_id, "Participants!A:D").doit().await;

    if let (Ok((_, q_range)), Ok((_, p_range))) = (quests_res, parts_res) {
        let q_rows = q_range.values.unwrap_or_default();
        let p_rows = p_range.values.unwrap_or_default();
        let now = chrono::Utc::now();

        use std::collections::HashMap;
        let mut quest_deadlines: HashMap<String, chrono::DateTime<chrono::FixedOffset>> = HashMap::new();

        for row in q_rows {
            if row.len() >= 8 {
                let q_id = row[0].as_str().unwrap_or("").to_string();
                let deadline_str = row[7].as_str().unwrap_or("");
                
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(deadline_str) {
                    quest_deadlines.insert(q_id, dt);
                }
            }
        }
        for (index, row) in p_rows.iter().enumerate() {
            if row.len() >= 4 {
                let q_id = row[0].as_str().unwrap_or("");
                let status = row[3].as_str().unwrap_or("");

                if status == "ON_PROGRESS" {
                    if let Some(deadline) = quest_deadlines.get(q_id) {
                        if now > *deadline {
                            println!("Quest {} expired for row {}. Marking FAILED.", q_id, index+1);

                            let row_number = index + 1;
                            let update_range = format!("Participants!D{}", row_number);
                             let req = ValueRange { 
                                values: Some(vec![vec![json!("FAILED")]]), 
                                ..Default::default() 
                            };
                            
                            let _ = hub.spreadsheets().values_update(req, spreadsheet_id, &update_range)
                                .value_input_option("RAW")
                                .doit().await;
                        }
                    }
                }
            }
        }
    }
    println!("Deadline Check Finished.");
}
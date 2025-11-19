use google_sheets4::{hyper, hyper_rustls, Sheets, chrono};
use google_sheets4::api::{ValueRange, BatchUpdateSpreadsheetRequest, Request, DeleteDimensionRequest, DimensionRange};
use serde_json::json;
use crate::models::{EventMessage, QuestPayload, RegistrationPayload, NewCommunityPayload, ProofPayload, EditPayload, DeletePayload};

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
                    json!(data.slots),
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

        "EDIT_QUEST" => {
            if let Ok(data) = serde_json::from_str::<EditPayload>(&event.payload) {
                match find_row_index(hub, spreadsheet_id, "Quests!A:A", &data.quest_id).await {
                    Ok(Some(row_number)) => {
                        // read full row so we can do a safe read-modify-write (avoid writing nulls)
                        let read_range = format!("Quests!A{}:J{}", row_number, row_number);
                        match hub.spreadsheets().values_get(spreadsheet_id, &read_range).doit().await {
                            Ok((_, vr)) => {
                                // prepare existing row with 10 columns (A..J)
                                let mut existing: Vec<String> = vec!["".to_string(); 10];
                                if let Some(rows) = vr.values {
                                    if let Some(first_row) = rows.get(0) {
                                        for (i, cell) in first_row.iter().enumerate().take(10) {
                                            existing[i] = cell.as_str().unwrap_or("").to_string();
                                        }
                                    }
                                }

                                // column mapping (0-based):
                                // 0: quest_id, 1: title, 2: category, 3: slots, 4: organizer_name,
                                // 5: schedule, 6: platform, 7: description, 8: deadline, 9: created_at
                                existing[1] = data.title;
                                existing[3] = data.slots.to_string();
                                existing[5] = data.schedule;
                                existing[6] = data.platform;
                                existing[7] = data.description;
                                existing[8] = data.deadline;

                                // write back the full updated row (A..J)
                                let values_json = vec![existing.iter().map(|s| serde_json::Value::String(s.clone())).collect::<Vec<_>>()];
                                let req = ValueRange { values: Some(values_json), ..Default::default() };
                                let write_range = read_range;
                                match hub.spreadsheets().values_update(req, spreadsheet_id, &write_range)
                                    .value_input_option("RAW")
                                    .doit().await {
                                    Ok(_) => println!("✅ Applied edit to quest {} at row {}", data.quest_id, row_number),
                                    Err(e) => eprintln!("Failed to apply edit to sheet: {:?}", e),
                                }
                            }
                            Err(e) => eprintln!("Failed to read existing quest row for {}: {:?}", data.quest_id, e),
                        }
                    }
                    Ok(None) => eprintln!("EDIT_QUEST: Quest id {} not found", data.quest_id),
                    Err(e) => eprintln!("EDIT_QUEST lookup error: {:?}", e),
                }
            } else {
                eprintln!("Failed to parse EDIT_QUEST payload");
            }
        },

        "DELETE_QUEST" => {
            if let Ok(data) = serde_json::from_str::<DeletePayload>(&event.payload) {
                match delete_quest_cascade(hub, spreadsheet_id, &data.quest_id).await {
                    Ok(_) => println!("✅ Cascade deleted quest {}", data.quest_id),
                    Err(e) => eprintln!("Failed to cascade delete quest {}: {:?}", data.quest_id, e),
                }
            } else {
                eprintln!("Failed to parse DELETE_QUEST payload");
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

        "DROP_QUEST" => {
            if let Ok(data) = serde_json::from_str::<RegistrationPayload>(&event.payload) {
                update_participant_status(hub, spreadsheet_id, &data.quest_id, &data.user_id, "DROPPED").await;
            }
        }

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

async fn find_row_index(hub: &HubType, spreadsheet_id: &str, range: &str, quest_id: &str) -> Result<Option<usize>, Box<dyn std::error::Error + Send + Sync>> {
    let res = hub.spreadsheets().values_get(spreadsheet_id, range).doit().await?;
    if let Some(vr) = res.1.values {
        for (i, row) in vr.iter().enumerate().skip(1) {
            if row.len() >= 1 && row[0].as_str().unwrap_or("") == quest_id {
                return Ok(Some(i + 1));
            }
        }
    }
    Ok(None)
}

pub async fn delete_quest_cascade(hub: &HubType, spreadsheet_id: &str, quest_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (_, meta) = hub.spreadsheets().get(spreadsheet_id).doit().await?;
    let sheets = meta.sheets.unwrap_or_default();

    let mut quests_sheet_id: Option<i32> = None;
    let mut participants_sheet_id: Option<i32> = None;
    for s in sheets {
        if let Some(props) = s.properties {
            match props.title.as_deref() {
                Some("Quests") => quests_sheet_id = props.sheet_id,
                Some("Participants") => participants_sheet_id = props.sheet_id,
                _ => {}
            }
        }
    }

    let q_sid = quests_sheet_id.ok_or("Quests sheet not found")?;
    let p_sid = participants_sheet_id.ok_or("Participants sheet not found")?;

    let q_rows = hub.spreadsheets().values_get(spreadsheet_id, "Quests!A:A").doit().await?;
    let mut quests_row_index: Option<usize> = None;
    if let Some(rows) = q_rows.1.values {
        for (i, row) in rows.iter().enumerate().skip(1) {
            if row.len() >= 1 && row[0].as_str().unwrap_or("") == quest_id {
                quests_row_index = Some(i);
                break;
            }
        }
    }

    if quests_row_index.is_none() {
        return Err(format!("Quest ID `{}` not found in Quests", quest_id).into());
    }

    let p_rows_res = hub.spreadsheets().values_get(spreadsheet_id, "Participants!A:A").doit().await?;
    let mut participant_row_indices: Vec<usize> = Vec::new();
    if let Some(rows) = p_rows_res.1.values {
        for (i, row) in rows.iter().enumerate().skip(1) {
            if row.len() >= 1 && row[0].as_str().unwrap_or("") == quest_id {
                participant_row_indices.push(i);
            }
        }
    }

    let mut requests: Vec<Request> = Vec::new();

    participant_row_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in participant_row_indices {
        let dr = DimensionRange {
            sheet_id: Some(p_sid),
            dimension: Some("ROWS".to_string()),
            start_index: Some(idx as i32),
            end_index: Some((idx + 1) as i32),
        };
        requests.push(Request { delete_dimension: Some(DeleteDimensionRequest { range: Some(dr) }), ..Default::default() });
    }

    let q_idx = quests_row_index.unwrap();
    let q_dr = DimensionRange {
        sheet_id: Some(q_sid),
        dimension: Some("ROWS".to_string()),
        start_index: Some(q_idx as i32),
        end_index: Some((q_idx + 1) as i32),
    };
    requests.push(Request { delete_dimension: Some(DeleteDimensionRequest { range: Some(q_dr) }), ..Default::default() });

    let batch = BatchUpdateSpreadsheetRequest { requests: Some(requests), ..Default::default() };
    hub.spreadsheets().batch_update(batch, spreadsheet_id).doit().await?;
    Ok(())
}
use serde::{Deserialize, Serialize};
use poise::ChoiceParameter;

#[derive(Debug, Serialize, Deserialize, ChoiceParameter, Clone, Copy)]
pub enum QuestCategory {
    #[name = "Creative Arts"]
    CreativeArts,
    #[name = "Community"]
    Community,
}

#[derive(Debug, Serialize, Deserialize, ChoiceParameter, Clone, Copy)]
pub enum Division {
    #[name = "Illustration"]
    Illust,
    #[name = "Game"]
    Game,
    #[name = "Music"]
    Music,
    #[name = "Talent Development"]
    Taldev,
    #[name = "Story"]
    Story,
    #[name = "Cosplay"]
    Cosplay,
    #[name = "- None -"]
    None,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuestPayload {
    pub quest_id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub organizer_name: String,
    pub slots: i8,
    pub schedule: String,
    pub platform: String,
    pub deadline: String,
    pub creator_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationPayload {
    pub quest_id: String,
    pub user_id: String,
    pub user_tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EditPayload {
    pub quest_id: String,
    pub title: String,
    pub description: String,
    pub slots: i8,
    pub schedule: String,
    pub deadline: String,
    pub platform: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeletePayload {
    pub quest_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewCommunityPayload {
    pub community_name: String,
    pub leader_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofPayload {
    pub quest_id: String,
    pub user_id: String,
    pub proof_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventMessage {
    pub event_type: String,
    pub payload: String,
}

#[derive(Debug, PartialEq)]
pub struct StatsResult {
    pub active: i32,
    pub completed: i32,
    pub failed: i32,
    pub list_str: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GuildConfig {
    pub announcement_channel_id: Option<u64>,
    pub ping_role_id: Option<u64>,
}
pub enum QuestCompleteMode {
    Take,
    Submit,
    View,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quest_category_serialization() {
        let cat = QuestCategory::CreativeArts;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"CreativeArts\"");
    }

    #[test]
    fn test_quest_payload_serialization() {
        let payload = QuestPayload {
            quest_id: "123".to_string(),
            title: "Test Quest".to_string(),
            description: "Desc".to_string(),
            category: "CreativeArts".to_string(),
            organizer_name: "Illust".to_string(),
            schedule: "2025-01-01T10:00:00+07:00".to_string(),
            platform: "Discord".to_string(),
            deadline: "2025-01-02T10:00:00+07:00".to_string(),
            creator_id: "999".to_string(),
            slots: 5,
        };
        
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"quest_id\":\"123\""));
        assert!(json.contains("\"slots\":5"));
    }

    #[test]
    fn test_event_message_wrapper() {
        let inner_payload = RegistrationPayload {
            quest_id: "q1".to_string(),
            user_id: "u1".to_string(),
            user_tag: "tag#1".to_string(),
        };
        let inner_json = serde_json::to_string(&inner_payload).unwrap();
        
        let event = EventMessage {
            event_type: "TAKE_QUEST".to_string(),
            payload: inner_json.clone(),
        };

        let event_json = serde_json::to_string(&event).unwrap();
        assert!(event_json.contains("TAKE_QUEST"));
        assert!(event_json.contains(&inner_json.replace("\"", "\\\"")));
    }
}
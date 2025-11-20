use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct EventMessage {
    pub event_type: String,
    pub payload: String,
}

#[derive(Debug, Deserialize)]
pub struct QuestPayload {
    pub quest_id: String,
    pub title: String,
    pub category: String,
    pub slots: i8,
    pub deadline: String,
    pub organizer_name: String,
    pub description: String,
    pub schedule: String,
    pub platform: String,
}

#[derive(Debug, Deserialize)]
pub struct RegistrationPayload {
    pub quest_id: String,
    pub user_id: String,
    pub user_tag: String,
}

#[derive(Debug, Deserialize)]
pub struct NewCommunityPayload {
    pub community_name: String,
    pub leader_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ProofPayload {
    pub quest_id: String,
    pub user_id: String,
    pub proof_url: String,
}

#[derive(Debug, Deserialize)]
pub struct EditPayload {
    pub quest_id: String,
    pub title: String,
    pub description: String,
    pub slots: i8,
    pub schedule: String,
    pub deadline: String,
    pub platform: String,
}

#[derive(Debug, Deserialize)]
pub struct DeletePayload {
    pub quest_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_quest_payload() {
        let json = r#"{
            "quest_id": "uuid-1",
            "title": "Mabar",
            "category": "Community",
            "organizer_name": "GenBalok",
            "description": "Main bareng",
            "schedule": "2025-11-20T19:00:00+07:00",
            "platform": "Discord",
            "deadline": "2025-11-20T21:00:00+07:00",
            "creator_id": "12345",
            "slots": 10
        }"#;

        let payload: QuestPayload = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(payload.quest_id, "uuid-1");
        assert_eq!(payload.slots, 10);
    }

    #[test]
    fn test_deserialize_event_message() {
        let json = r#"{
            "event_type": "CREATE_QUEST",
            "payload": "{\"some\":\"inner_json\"}"
        }"#;
        
        let event: EventMessage = serde_json::from_str(json).expect("Should deserialize wrapper");
        assert_eq!(event.event_type, "CREATE_QUEST");
        assert_eq!(event.payload, "{\"some\":\"inner_json\"}");
    }
}
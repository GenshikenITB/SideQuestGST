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
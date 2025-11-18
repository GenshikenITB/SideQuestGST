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
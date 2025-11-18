# GST25 Side Quest Platform - System Design Document

## 1. Overview

Platform ini dirancang untuk mengotomatisasi alur kerja "Side Quest" pada event CaStaff GST25. Sistem bertindak sebagai jembatan antara user interface (Discord) dan database administratif (Google Sheets), menggunakan pendekatan event-driven.

## 2. Tech Stack

- Programming Language: Rust (Edition 2021)

     - Alasan: Type-safe, Memory-safe (Borrow checker), High performance, Compiled.

- Discord Framework: `poise` (wrapper di atas `serenity`).

- Message Broker: Apache Kafka (Mode KRaft - tanpa Zookeeper untuk efisiensi).

- Database: Google Sheets API v4.

- Container: Docker (Multistage build).

- Orchestration: Kubernetes (Deployment, Service, ConfigMap).

## 3. Service Architecture

### A. Discord Gateway Service (`gst-quest-bot`)

Service ini bertugas sebagai "Front-end" yang berhadapan langsung dengan user.

- Tanggung Jawab:

    - Mendengarkan Slash Commands (`/create_quest, /take_quest, /submit_proof`).

    - Memvalidasi Role (Staff vs CaStaff).

    - Memvalidasi Server ID (GST Server Only).

    - Menampilkan Form (Discord Modals) untuk input data.

    - Mengirim Embed ke channel pengumuman.

    - Producer: Mengirim event ke Kafka Topic saat ada aksi user.

### B. Sheet Worker Service (`gst-sheet-worker`)

Service ini bertugas sebagai "Back-end" worker.

- Tanggung Jawab:

    - Consumer: Mendengarkan Kafka Topics.

    - Melakukan writing ke Google Spreadsheet.

    - Error handling jika API Google Sheets down (retry logic).

## 4. Kafka Topics & Data Flow

Topic: `quest.events`

Semua event dikirim ke topic ini dengan `key` yang berbeda untuk segregasi tipe event.

### A. Pembuatan Quest (Staff)

1. User: Mengetik `/create_quest` di Discord.

2. Bot: Memunculkan Modal (Form) berisi: Judul, Deskripsi, Tipe (Karya/Komunitas), Deadline.

3. Bot:

    - Generate `QuestToken` (UUID).

    - Kirim Embed ke channel #quest-board.

    - Produce message ke Kafka:

    ```JSON
    {
    "event_type": "CREATE_QUEST",
    "payload": {
        "quest_id": "uuid-v4",
        "creator_id": "discord_user_id",
        "title": "Mabar MLBB",
        "description": "Push rank bareng",
        "category": "COMMUNITY"
    }
    }
    ```


### B. Pengambilan Quest (CaStaff)

1. User: Klik tombol "Ambil Quest" pada Embed atau ketik `/take_quest <quest_id>`.

2. Bot: Validasi apakah user CaStaff.

3. Bot: Produce message ke Kafka:
    ```JSON
    {
    "event_type": "TAKE_QUEST",
    "payload": {
        "quest_id": "uuid-v4",
        "user_id": "discord_user_id",
        "user_name": "DiscordTag#1234",
        "timestamp": "2025-11-21T10:00:00Z"
    }
    }
    ```

### C. Pengumpulan Bukti (CaStaff)

1. User: Mengetik `/submit_proof` (Bot akan mengirim DM berisi form/attachment upload).

2. Bot: Menerima gambar/link bukti.

3. Bot: Produce message ke Kafka:

    ```JSON
    {
    "event_type": "SUBMIT_PROOF",
    "payload": {
        "quest_id": "uuid-v4",
        "user_id": "discord_user_id",
        "proof_url": "[https://cdn.discordapp.com/](https://cdn.discordapp.com/)...",
        "timestamp": "2025-11-21T12:00:00Z"
    }
    }
    ```

## 5. Google Sheets Schema

Worker akan menulis ke Spreadsheet dengan Tab terpisah:

1. Tab `Quests`: ID, Judul, Deskripsi, Kategori, Creator, CreatedAt.

2. Tab `Participants`: QuestID, UserID, UserName, Status (Taken/Submitted), ProofURL, Timestamp.

## 6. Security & Validation

- Guild Lock: Middleware di Rust akan mengecek `ctx.guild_id()` == `GST_SERVER_ID`.

- Role Check:

    - `create_quest`: Hanya role `QuestMaster`.

    - `take_quest`: Hanya role `Castaff`.
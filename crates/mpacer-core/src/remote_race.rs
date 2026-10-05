//! Course a distance ("remote race") : protocole et classement temps reel.
//!
//! Le transport reseau est hors du coeur : le shell Android (ou un service
//! WebSocket) pousse des `RaceMessage` et le moteur tient le classement.
//! Cela rend la logique testable sans reseau.

use serde::{Deserialize, Serialize};

/// Phase de la course.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RacePhase {
    /// Salle d'attente : les coureurs rejoignent la course.
    Lobby,
    /// Tous les coureurs ont signale "pret".
    Ready,
    /// Decompte avant le depart.
    Countdown {
        remaining_s: u8,
    },
    Running,
    Finished,
}

/// Un concurrent et sa position connue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceParticipant {
    pub nickname: String,
    pub distance_m: f64,
    pub elapsed_s: f64,
    pub finished: bool,
    pub finish_time_s: Option<f64>,
}

/// Message echange avec le serveur de course.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RaceMessage {
    Join {
        race: String,
        nickname: String,
        distance_m: f64,
    },
    Left {
        nickname: String,
    },
    Ready {
        nickname: String,
    },
    /// Position publiee toutes les quelques secondes.
    Position {
        nickname: String,
        distance_m: f64,
        elapsed_s: f64,
    },
    Finish {
        nickname: String,
        time_s: f64,
    },
    /// Le serveur confirme la composition et lance le decompte.
    StartRace {
        countdown_s: u8,
    },
}

/// Ligne du classement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceStanding {
    pub rank: u32,
    pub nickname: String,
    pub distance_m: f64,
    /// Ecart avec le meneur (m).
    pub gap_to_leader_m: f64,
    pub finished: bool,
}

/// Session de course a distance cote client.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteRaceSession {
    pub name: String,
    pub distance_m: f64,
    pub local_nickname: String,
    pub phase: RacePhase,
    pub participants: Vec<RaceParticipant>,
    ready: Vec<String>,
}

impl RemoteRaceSession {
    pub fn new(
        name: impl Into<String>,
        distance_m: f64,
        local_nickname: impl Into<String>,
    ) -> Self {
        let local = local_nickname.into();
        Self {
            name: name.into(),
            distance_m,
            participants: vec![RaceParticipant {
                nickname: local.clone(),
                distance_m: 0.0,
                elapsed_s: 0.0,
                finished: false,
                finish_time_s: None,
            }],
            local_nickname: local,
            phase: RacePhase::Lobby,
            ready: Vec::new(),
        }
    }

    /// Applique un message recu du serveur.
    pub fn apply(&mut self, message: RaceMessage) {
        match message {
            RaceMessage::Join { nickname, .. } => self.join(nickname),
            RaceMessage::Left { nickname } => {
                self.participants.retain(|p| p.nickname != nickname);
                self.ready.retain(|n| n != &nickname);
            }
            RaceMessage::Ready { nickname } => self.mark_ready(&nickname),
            RaceMessage::Position {
                nickname,
                distance_m,
                elapsed_s,
            } => self.update_position(&nickname, distance_m, elapsed_s),
            RaceMessage::Finish { nickname, time_s } => self.finish(&nickname, time_s),
            RaceMessage::StartRace { countdown_s } => {
                self.phase = RacePhase::Countdown {
                    remaining_s: countdown_s,
                };
            }
        }
    }

    pub fn join(&mut self, nickname: impl Into<String>) {
        let nickname = nickname.into();
        if self.participants.iter().any(|p| p.nickname == nickname) {
            return;
        }
        self.participants.push(RaceParticipant {
            nickname,
            distance_m: 0.0,
            elapsed_s: 0.0,
            finished: false,
            finish_time_s: None,
        });
    }

    /// Marque un coureur pret ; passe en phase `Ready` si tout le monde l'est.
    pub fn mark_ready(&mut self, nickname: &str) {
        if !self.ready.iter().any(|n| n == nickname) {
            self.ready.push(nickname.to_string());
        }
        let all_ready = !self.participants.is_empty()
            && self
                .participants
                .iter()
                .all(|p| self.ready.iter().any(|n| n == &p.nickname));
        if all_ready && self.phase == RacePhase::Lobby {
            self.phase = RacePhase::Ready;
        }
    }

    pub fn is_ready(&self, nickname: &str) -> bool {
        self.ready.iter().any(|n| n == nickname)
    }

    /// Position locale (celle de l'utilisateur).
    pub fn update_local_position(&mut self, distance_m: f64, elapsed_s: f64) {
        let local = self.local_nickname.clone();
        self.update_position(&local, distance_m, elapsed_s);
    }

    /// Position d'un concurrent, avec detection d'arrivee.
    pub fn update_position(&mut self, nickname: &str, distance_m: f64, elapsed_s: f64) {
        if let Some(participant) = self
            .participants
            .iter_mut()
            .find(|p| p.nickname == nickname)
        {
            participant.distance_m = distance_m;
            participant.elapsed_s = elapsed_s;
            if distance_m >= self.distance_m && !participant.finished {
                participant.finished = true;
                participant.finish_time_s = Some(elapsed_s);
            }
        }
        if self.phase == RacePhase::Lobby || self.phase == RacePhase::Ready {
            self.phase = RacePhase::Running;
        }
    }

    pub fn finish(&mut self, nickname: &str, time_s: f64) {
        if let Some(participant) = self
            .participants
            .iter_mut()
            .find(|p| p.nickname == nickname)
        {
            participant.finished = true;
            participant.finish_time_s = Some(time_s);
        }
        if !self.participants.is_empty() && self.participants.iter().all(|p| p.finished) {
            self.phase = RacePhase::Finished;
        }
    }

    /// Classement courant (distance decroissante, arrivants en tete).
    pub fn standings(&self) -> Vec<RaceStanding> {
        let mut sorted: Vec<&RaceParticipant> = self.participants.iter().collect();
        sorted.sort_by(|a, b| {
            let key = |p: &RaceParticipant| {
                let progress = (p.distance_m / self.distance_m.max(1.0)).clamp(0.0, 1.0);
                (p.finished, progress)
            };
            key(b)
                .partial_cmp(&key(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let leader_distance = sorted.first().map(|p| p.distance_m).unwrap_or(0.0);
        sorted
            .iter()
            .enumerate()
            .map(|(index, participant)| RaceStanding {
                rank: index as u32 + 1,
                nickname: participant.nickname.clone(),
                distance_m: participant.distance_m,
                gap_to_leader_m: (leader_distance - participant.distance_m).max(0.0),
                finished: participant.finished,
            })
            .collect()
    }

    /// Position locale dans le classement.
    pub fn local_rank(&self) -> Option<u32> {
        self.standings()
            .into_iter()
            .find(|s| s.nickname == self.local_nickname)
            .map(|s| s.rank)
    }

    /// Ecart (m) avec un adversaire : positif si je suis devant.
    pub fn delta_to(&self, nickname: &str) -> Option<f64> {
        let me = self
            .participants
            .iter()
            .find(|p| p.nickname == self.local_nickname)?;
        let other = self.participants.iter().find(|p| p.nickname == nickname)?;
        Some(me.distance_m - other.distance_m)
    }

    /// Pseudo de l'adversaire (premier concurrent autre que moi).
    pub fn opponent(&self) -> Option<&str> {
        self.participants
            .iter()
            .find(|p| p.nickname != self.local_nickname)
            .map(|p| p.nickname.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_starts_in_lobby_with_local_runner() {
        let session = RemoteRaceSession::new("sunday", 10_000.0, "joseph");
        assert_eq!(session.phase, RacePhase::Lobby);
        assert_eq!(session.participants.len(), 1);
        assert_eq!(session.local_rank(), Some(1));
    }

    #[test]
    fn both_runners_ready_moves_to_ready_phase() {
        let mut session = RemoteRaceSession::new("sunday", 10_000.0, "joseph");
        session.apply(RaceMessage::Join {
            race: "sunday".into(),
            nickname: "paul".into(),
            distance_m: 10_000.0,
        });
        session.apply(RaceMessage::Ready {
            nickname: "joseph".into(),
        });
        assert_eq!(session.phase, RacePhase::Lobby);
        session.apply(RaceMessage::Ready {
            nickname: "paul".into(),
        });
        assert_eq!(session.phase, RacePhase::Ready);
    }

    #[test]
    fn standings_follow_positions() {
        let mut session = RemoteRaceSession::new("sunday", 10_000.0, "joseph");
        session.join("paul");
        session.update_local_position(3000.0, 900.0);
        session.apply(RaceMessage::Position {
            nickname: "paul".into(),
            distance_m: 3200.0,
            elapsed_s: 900.0,
        });
        let standings = session.standings();
        assert_eq!(standings[0].nickname, "paul");
        assert_eq!(session.local_rank(), Some(2));
        assert!((session.delta_to("paul").unwrap() + 200.0).abs() < 1e-9);
        assert_eq!(session.opponent(), Some("paul"));
    }

    #[test]
    fn finishing_all_runners_ends_the_race() {
        let mut session = RemoteRaceSession::new("sunday", 5000.0, "joseph");
        session.join("paul");
        session.apply(RaceMessage::Position {
            nickname: "paul".into(),
            distance_m: 5000.0,
            elapsed_s: 1500.0,
        });
        session.apply(RaceMessage::Finish {
            nickname: "joseph".into(),
            time_s: 1520.0,
        });
        assert_eq!(session.phase, RacePhase::Finished);
        let standings = session.standings();
        assert!(standings[0].finished);
    }

    #[test]
    fn leaving_runner_is_removed() {
        let mut session = RemoteRaceSession::new("sunday", 5000.0, "joseph");
        session.join("paul");
        session.apply(RaceMessage::Left {
            nickname: "paul".into(),
        });
        assert_eq!(session.participants.len(), 1);
        assert_eq!(session.opponent(), None);
    }
}

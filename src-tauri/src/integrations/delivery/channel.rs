/// Canonical channel names stored in `integration_deliveries`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryChannel {
    Smtp,
    Signal,
    Telegram,
    Gmail,
    GoogleDocs,
    GoogleDrive,
    GoogleCalendar,
    GoogleTasks,
}

impl DeliveryChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smtp => "SMTP",
            Self::Signal => "SIGNAL",
            Self::Telegram => "TELEGRAM",
            Self::Gmail => "GMAIL",
            Self::GoogleDocs => "GDOCS",
            Self::GoogleDrive => "DRIVE",
            Self::GoogleCalendar => "GCALENDAR",
            Self::GoogleTasks => "GTASKS",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "SMTP" => Some(Self::Smtp),
            "SIGNAL" => Some(Self::Signal),
            "TELEGRAM" => Some(Self::Telegram),
            "GMAIL" => Some(Self::Gmail),
            "GDOCS" => Some(Self::GoogleDocs),
            "DRIVE" => Some(Self::GoogleDrive),
            "GCALENDAR" => Some(Self::GoogleCalendar),
            "GTASKS" => Some(Self::GoogleTasks),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_channel() {
        for channel in [
            DeliveryChannel::Smtp,
            DeliveryChannel::Signal,
            DeliveryChannel::Telegram,
            DeliveryChannel::Gmail,
            DeliveryChannel::GoogleDocs,
            DeliveryChannel::GoogleDrive,
            DeliveryChannel::GoogleCalendar,
            DeliveryChannel::GoogleTasks,
        ] {
            assert_eq!(DeliveryChannel::parse(channel.as_str()), Some(channel));
        }
        assert_eq!(DeliveryChannel::parse("WEBHOOK"), None);
    }
}

#![forbid(unsafe_code)]

//! Identify by endpoint: whatever arrives here is from whom the Receive
//! Location's configuration says it is from.
//!
//! Some channels have exactly one far end and no way for it to say who it is:
//! a drop folder one partner writes to, a leased line, a mailbox Xmip empties
//! on a schedule, a serial port with one instrument on it. The operator knows
//! who is there because the operator put them there, and says so once in the
//! Receive Location's configuration. This identifier is built from that
//! sentence and presents it for every Stream that arrives on the Location,
//! under [`xcore::mechanism::endpoint`].
//!
//! The claim is **inferred**, never passed: nobody presented anything, and
//! the record says so. It is presented however the Stream arrived — pushed,
//! detected or scheduled — because the configuration is as true of a pickup
//! as of a connection, and a scheduled pickup can yield no other kind of
//! identity. Nothing the sender puts on the arrival changes the value; a
//! header naming somebody else is another identifier's claim, and the two
//! standing side by side on the record is what a dispute needs.
//!
//! Evidence this technology writes: `endpoint.source`, the URI the Stream
//! came from, and `endpoint.arriving`, how it got here. It reads no property
//! and attaches no proof: there is nothing to prove, and the second gate's
//! answer for an inferred claim is the Location's acceptance of it.

use identify::{IdentifyError, Presented, StreamArrival, TransportIdentifier};
use xcore::Mechanism;

/// The evidence name the arrival's source URI rides under.
pub const SOURCE: &str = "endpoint.source";
/// The evidence name the way the Stream arrived rides under.
pub const ARRIVING: &str = "endpoint.arriving";

/// Presents the identity one Receive Location's configuration names.
#[derive(Clone, Debug)]
pub struct Endpoint {
    identity: String,
}

impl Endpoint {
    /// Whatever arrives on the Location is from this identity.
    ///
    /// # Errors
    ///
    /// Where the configuration names nobody: an empty identity would put a
    /// claim with no claimant on every record, so it is refused when the
    /// identifier is built.
    pub fn named(identity: &str) -> Result<Self, IdentifyError> {
        let identity = identity.trim();
        if identity.is_empty() {
            return Err(IdentifyError::new(
                "the Receive Location's configuration names no identity for its endpoint",
            ));
        }

        Ok(Self {
            identity: identity.to_string(),
        })
    }

    /// The identity the configuration names.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

impl TransportIdentifier for Endpoint {
    fn mechanism(&self) -> Mechanism {
        xcore::mechanism::endpoint()
    }

    fn identify(&self, arrival: &StreamArrival<'_>) -> Result<Option<Presented>, IdentifyError> {
        Ok(Some(
            Presented::inferred(self.mechanism(), &self.identity)
                .with_evidence(SOURCE, arrival.source_uri())
                .with_evidence(ARRIVING, arrival.arriving().to_string()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stream::Stream;
    use xcore::{Arriving, Established, Layer, StreamId};

    fn stream() -> Stream {
        Stream::new(StreamId::new(1), b"<order/>".to_vec(), None)
    }

    fn partner() -> Endpoint {
        Endpoint::named("partner-x").expect("an identity")
    }

    #[test]
    fn what_arrives_on_the_location_is_from_whom_the_configuration_says() {
        let stream = stream();
        let arrival = StreamArrival::new(&stream, Arriving::Detected, "file:///in/partner-x", &[]);

        let claim = partner()
            .identify(&arrival)
            .expect("read")
            .expect("a claim");

        assert_eq!(claim.mechanism.name(), "endpoint");
        assert_eq!(claim.value, "partner-x");
        assert_eq!(claim.established, Established::Inferred);
        assert_eq!(claim.layer(), Layer::Transport);
        assert!(
            claim
                .evidence
                .contains(&(SOURCE.to_string(), "file:///in/partner-x".to_string()))
        );
    }

    #[test]
    fn a_scheduled_pickup_is_identified_although_nobody_was_there_to_pass_anything() {
        let stream = stream();

        for arriving in [Arriving::Pushed, Arriving::Detected, Arriving::Scheduled] {
            let arrival = StreamArrival::new(&stream, arriving, "sftp://partner/out", &[]);

            let claim = partner()
                .identify(&arrival)
                .expect("read")
                .expect("a claim");

            assert_eq!(claim.established, Established::Inferred);
            assert!(
                claim
                    .evidence
                    .contains(&(ARRIVING.to_string(), arriving.to_string()))
            );
        }
    }

    #[test]
    fn nothing_the_sender_puts_on_the_arrival_changes_the_inferred_identity() {
        let stream = stream();
        let facts = [
            (
                "http.header.x-partner-id".to_string(),
                "mallory".to_string(),
            ),
            ("endpoint".to_string(), "mallory".to_string()),
        ];
        let arrival = StreamArrival::new(&stream, Arriving::Pushed, "https://xmip/in", &facts);

        let claim = partner()
            .identify(&arrival)
            .expect("read")
            .expect("a claim");

        assert_eq!(claim.value, "partner-x");
        assert!(claim.evidence.iter().all(|(_, value)| value != "mallory"));
    }

    #[test]
    fn a_configuration_naming_nobody_is_refused_when_the_identifier_is_built() {
        let failure = Endpoint::named("  ").expect_err("nobody");

        assert_eq!(
            failure.to_string(),
            "the Receive Location's configuration names no identity for its endpoint"
        );
    }

    #[test]
    fn the_configured_identity_is_trimmed_and_attaches_no_proof() {
        let stream = stream();
        let arrival = StreamArrival::new(&stream, Arriving::Detected, "file:///in/x", &[]);
        let endpoint = Endpoint::named(" partner-x ").expect("an identity");

        let claim = endpoint.identify(&arrival).expect("read").expect("a claim");

        assert_eq!(endpoint.identity(), "partner-x");
        assert!(!claim.mechanism.authenticates());
        assert!(format!("{claim:?}").contains("proof: []"));
    }
}

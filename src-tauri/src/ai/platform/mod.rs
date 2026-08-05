//! Capability-driven AI provider platform (RC3.5).
//!
//! Descriptors, protocol families, auth modes, endpoint classes, and capability
//! profiles are the trusted source of product behavior. Adapters implement a
//! protocol family; presets supply official endpoints and metadata.

mod descriptors;
mod endpoint_policy;
mod types;

pub use descriptors::{
    descriptor_by_id, list_consumer_descriptors, list_descriptors, ProviderDescriptor,
    PROVIDER_PRESET_VERSION,
};
pub use endpoint_policy::{
    build_redirect_free_pinned_client, classify_and_validate_endpoint,
    classify_and_validate_endpoint_with_resolver, is_blocked_destination_ip,
    validate_and_build_credential_client, EndpointPolicyError, EndpointValidation,
};
pub use types::{
    AuthMode, CapabilityConfidence, CapabilityFlag, CapabilityProfile, ConnectionHealth,
    EndpointClass, ModelDiscoveryStrategy, ProtocolFamily,
};

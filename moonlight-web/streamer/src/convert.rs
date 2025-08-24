use common::api_bindings::{RtcIceServer, RtcSdpType};
use webrtc::{ice_transport::ice_server::RTCIceServer, peer_connection::sdp::sdp_type::RTCSdpType};

pub fn into_webrtc_sdp(value: RtcSdpType) -> RTCSdpType {
    match value {
        RtcSdpType::Offer => RTCSdpType::Offer,
        RtcSdpType::Answer => RTCSdpType::Answer,
        RtcSdpType::Pranswer => RTCSdpType::Pranswer,
        RtcSdpType::Rollback => RTCSdpType::Rollback,
        RtcSdpType::Unspecified => RTCSdpType::Unspecified,
    }
}

pub fn from_webrtc_sdp(value: RTCSdpType) -> RtcSdpType {
    match value {
        RTCSdpType::Offer => RtcSdpType::Offer,
        RTCSdpType::Answer => RtcSdpType::Answer,
        RTCSdpType::Pranswer => RtcSdpType::Pranswer,
        RTCSdpType::Rollback => RtcSdpType::Rollback,
        RTCSdpType::Unspecified => RtcSdpType::Unspecified,
    }
}

pub fn into_webrtc_ice(value: RtcIceServer) -> RTCIceServer {
    RTCIceServer {
        urls: value.urls,
        username: value.username,
        credential: value.credential,
    }
}

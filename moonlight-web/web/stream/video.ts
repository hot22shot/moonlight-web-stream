import { StreamSupportedVideoFormats } from "../api_bindings.js"

export type VideoCodecSupport = {
    H264: boolean
    H264_HIGH8_444: boolean
    H265: boolean
    H265_MAIN10: boolean
    H265_REXT8_444: boolean
    H265_REXT10_444: boolean
    AV1_MAIN8: boolean
    AV1_MAIN10: boolean
    AV1_HIGH8_444: boolean
    AV1_HIGH10_444: boolean
} & Record<string, boolean>

const CODECS: Array<{ key: string } & VideoDecoderConfig> = [
    { key: "H264_HIGH8_444", codec: "avc1.4d400c", colorSpace: { primaries: "bt709", matrix: "bt709", transfer: "bt709", fullRange: true } },
    { key: "H265", codec: "hvc1.1.6.L93.B0" },
    { key: "H265_MAIN10", codec: "hvc1.2.4.L120.90" },
    { key: "H265_REXT8_444", codec: "hvc1.6.6.L93.90", colorSpace: { primaries: "bt709", matrix: "bt709", transfer: "bt709", fullRange: true } },
    { key: "H265_REXT10_444", codec: "hvc1.6.10.L120.90", colorSpace: { primaries: "bt709", matrix: "bt709", transfer: "bt709", fullRange: true } },
    { key: "AV1_MAIN8", codec: "av01.0.04M.08" },
    { key: "AV1_MAIN10", codec: "av01.0.04M.10" },
    { key: "AV1_HIGH8_444", codec: "av01.0.08M.08", colorSpace: { primaries: "bt709", matrix: "bt709", transfer: "bt709", fullRange: true } },
    { key: "AV1_HIGH10_444", codec: "av01.0.08M.10", colorSpace: { primaries: "bt709", matrix: "bt709", transfer: "bt709", fullRange: true } }
]

export async function getSupportedVideoFormats(): Promise<VideoCodecSupport> {
    let support: VideoCodecSupport = {
        H264: true,              // assumed universal
        H264_HIGH8_444: false,
        H265: false,
        H265_MAIN10: false,
        H265_REXT8_444: false,
        H265_REXT10_444: false,
        AV1_MAIN8: false,
        AV1_MAIN10: false,
        AV1_HIGH8_444: false,
        AV1_HIGH10_444: false
    }

    if ("VideoDecoder" in window && window.isSecureContext) {
        for (const codec of CODECS) {
            try {
                const result = await VideoDecoder.isConfigSupported(codec)

                support[codec.key] = result.supported || support[codec.key]
            } catch (e) {
                support[codec.key] = false
            }
        }
    } else if ("MediaSource" in window) {
        for (const codec of CODECS) {
            const supported = MediaSource.isTypeSupported(`video/mp4; codecs="${codec.codec}"`)

            support[codec.key] = supported || support[codec.key]
        }
    } else {
        const mediaElement = document.createElement("video")

        for (const codec of CODECS) {
            const supported = mediaElement.canPlayType(`video/mp4; codecs="${codec.codec}"`)

            support[codec.key] = supported == "probably" || support[codec.key]
        }
    }

    return support
}

export function createSupportedVideoFormatsBits(support: VideoCodecSupport): number {
    let mask = 0

    if (support.H264) {
        mask |= StreamSupportedVideoFormats.H264
    }
    if (support.H264_HIGH8_444) {
        mask |= StreamSupportedVideoFormats.H264_HIGH8_444
    }
    if (support.H265) {
        mask |= StreamSupportedVideoFormats.H265
    }
    if (support.H265_MAIN10) {
        mask |= StreamSupportedVideoFormats.H265_MAIN10
    }
    if (support.H265_REXT8_444) {
        mask |= StreamSupportedVideoFormats.H265_REXT8_444
    }
    if (support.H265_REXT10_444) {
        mask |= StreamSupportedVideoFormats.H265_REXT10_444
    }
    if (support.AV1_MAIN8) {
        mask |= StreamSupportedVideoFormats.AV1_MAIN8
    }
    if (support.AV1_MAIN10) {
        mask |= StreamSupportedVideoFormats.AV1_MAIN10
    }
    if (support.AV1_HIGH8_444) {
        mask |= StreamSupportedVideoFormats.AV1_HIGH8_444
    }
    if (support.AV1_HIGH10_444) {
        mask |= StreamSupportedVideoFormats.AV1_HIGH10_444
    }

    return mask
}
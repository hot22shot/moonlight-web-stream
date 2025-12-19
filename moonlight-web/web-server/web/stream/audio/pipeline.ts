import { AudioPlayer, DataAudioPlayer, TrackAudioPlayer } from "./index.js"
import { AudioDecoderPipe } from "./audio_decoder_pipe.js"
import { AudioElementPlayer } from "./audio_element.js"
import { AudioMediaStreamTrackGeneratorPipe } from "./media_stream_track_generator_pipe.js"

type PipelineResult<T> = { audioPlayer: T, log: string, error: null } | { audioPlayer: null, log: string, error: string }

interface FinalAudioRenderer {
    new(): AudioPlayer

    readonly type: string
    isBrowserSupported(): boolean
}
const FINAL_AUDIO_RENDERER: Array<FinalAudioRenderer> = [
    AudioElementPlayer
]

interface AudioPipe {
    new(base: any): AudioPlayer

    readonly type: string
    isBrowserSupported(): boolean
}
const PIPE_TYPES: Array<string> = ["data", "audiotrack", "audiosample"]
const AUDIO_PIPES: Record<string, AudioPipe> = {
    data_to_audiosample: AudioDecoderPipe,
    audiotrack_to_audiosample: AudioMediaStreamTrackGeneratorPipe,
}

export type AudioPipelineOptions = {
}

export function buildAudioPipeline(type: "audiotrack", settings: AudioPipelineOptions): PipelineResult<TrackAudioPlayer>
export function buildAudioPipeline(type: "data", settings: AudioPipelineOptions): PipelineResult<DataAudioPlayer>

export function buildAudioPipeline(type: string, settings: AudioPipelineOptions): PipelineResult<AudioPlayer> {
    let log = `Building audio pipeline with output "${type}"`

    // TODO dynamically create pipelines based on browser support

    if (type == "audiotrack") {
        if (AudioElementPlayer.isBrowserSupported()) {
            const audioPlayer = new AudioElementPlayer()

            return { audioPlayer, log, error: null }
        } else {
            return { audioPlayer: null, log, error: "AudioElementPlayer is not supported -> cannot play audio" }
        }
    } else if (type == "data") {
        if (AudioDecoderPipe.isBrowserSupported() && AudioMediaStreamTrackGeneratorPipe.isBrowserSupported() && AudioElementPlayer.isBrowserSupported()) {
            const audioPlayer = new AudioDecoderPipe(new AudioMediaStreamTrackGeneratorPipe(new AudioElementPlayer()))

            return { audioPlayer, log, error: null }
        } else {
            return { audioPlayer: null, log, error: `One of AudioDecoder,AudioMediaStreamTrackGenerator,AudioElementPlayer is not supported -> cannot play audio` }
        }
    }

    return { audioPlayer: null, log, error: "No supported audio player found!" }
}
import { ControllerConfig } from "../stream/gamepad.js";
import { Component, ComponentEvent } from "./index.js";
import { InputComponent } from "./input.js";

export type StreamSettings = {
    bitrate: number
    packetSize: number
    videoSize?: {
        width: number
        height: number
    },
    fps: number
    controllerConfig: ControllerConfig
}

export function defaultStreamSettings(): StreamSettings {
    return {
        bitrate: 5000,
        packetSize: 4096,
        fps: 60,
        controllerConfig: {
            invertAB: false,
            invertXY: false
        }
    }
}

export function getLocalStreamSettings(): StreamSettings | null {
    let settings = null
    try {
        const settingsLoadedJson = localStorage.getItem("mlSettings")
        if (settingsLoadedJson == null) {
            return null
        }

        const settingsLoaded = JSON.parse(settingsLoadedJson)

        settings = defaultStreamSettings()
        Object.assign(settings, settingsLoaded)
    } catch (e) {
        localStorage.removeItem("mlSettings")
    }
    return settings
}
export function setLocalStreamSettings(settings?: StreamSettings) {
    localStorage.setItem("mlSettings", JSON.stringify(settings))
}

export type StreamSettingsChangeListener = (event: ComponentEvent<StreamSettingsComponent>) => void

export class StreamSettingsComponent implements Component {

    private divElement: HTMLDivElement = document.createElement("div")

    // TODO: move these to the input component
    private streamHeader: HTMLHeadingElement = document.createElement("h2")
    private bitrate: InputComponent
    private packetSize: InputComponent
    private fps: InputComponent

    private videoSizeEnabled: InputComponent
    private videoSizeWidth: InputComponent
    private videoSizeHeight: InputComponent

    private controllerHeader: HTMLHeadingElement = document.createElement("h2")
    private controllerInvertAB: InputComponent
    private controllerInvertXY: InputComponent

    constructor(settings?: StreamSettings) {
        const defaultSettings = defaultStreamSettings()

        // Root div
        this.divElement.classList.add("settings")

        this.streamHeader.innerText = "Streaming"
        this.divElement.appendChild(this.streamHeader)

        // Bitrate
        this.bitrate = new InputComponent("bitrate", "number", "Bitrate", {
            defaultValue: defaultSettings.bitrate.toString(),
            value: settings?.bitrate?.toString(),
            step: "100",
        })
        this.bitrate.addChangeListener(this.onSettingsChange.bind(this))
        this.bitrate.mount(this.divElement)

        // Packet Size
        this.packetSize = new InputComponent("packetSize", "number", "Packet Size", {
            defaultValue: defaultSettings.packetSize.toString(),
            value: settings?.packetSize?.toString(),
            step: "100"
        })
        this.packetSize.addChangeListener(this.onSettingsChange.bind(this))
        this.packetSize.mount(this.divElement)

        // Fps
        this.fps = new InputComponent("fps", "number", "Fps", {
            defaultValue: defaultSettings.fps.toString(),
            value: settings?.fps?.toString(),
            step: "100"
        })
        this.fps.addChangeListener(this.onSettingsChange.bind(this))
        this.fps.mount(this.divElement)

        // Video Size
        this.videoSizeEnabled = new InputComponent("videoSizeEnabled", "checkbox", "Fixed Video Size", {
            checked: settings?.videoSize != null
        })
        this.videoSizeEnabled.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSizeEnabled.mount(this.divElement)

        this.videoSizeWidth = new InputComponent("videoSizeWidth", "number", "Video Width", {
            defaultValue: "1920",
            value: settings?.videoSize?.width?.toString()
        })
        this.videoSizeWidth.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSizeWidth.mount(this.divElement)

        this.videoSizeHeight = new InputComponent("videoSizeHeight", "number", "Video Height", {
            defaultValue: "1080",
            value: settings?.videoSize?.height?.toString()
        })
        this.videoSizeHeight.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSizeHeight.mount(this.divElement)

        // Controller
        this.controllerHeader.innerText = "Controller"
        this.divElement.appendChild(this.controllerHeader)

        this.controllerInvertAB = new InputComponent("controllerInvertAB", "checkbox", "Invert A and B", {
            checked: settings?.controllerConfig.invertAB
        })
        this.controllerInvertAB.addChangeListener(this.onSettingsChange.bind(this))
        this.controllerInvertAB.mount(this.divElement)

        this.controllerInvertXY = new InputComponent("controllerInvertXY", "checkbox", "Invert X and Y", {
            checked: settings?.controllerConfig.invertXY
        })
        this.controllerInvertXY.addChangeListener(this.onSettingsChange.bind(this))
        this.controllerInvertXY.mount(this.divElement)

        this.onSettingsChange()
    }

    private onSettingsChange() {
        if (this.videoSizeEnabled.isChecked()) {
            this.videoSizeWidth.setEnabled(true)
            this.videoSizeHeight.setEnabled(true)
        } else {
            this.videoSizeWidth.setEnabled(false)
            this.videoSizeHeight.setEnabled(false)
        }

        this.divElement.dispatchEvent(new ComponentEvent("ml-settingschange", this))
    }

    addChangeListener(listener: StreamSettingsChangeListener) {
        this.divElement.addEventListener("ml-settingschange", listener as any)
    }
    removeChangeListener(listener: StreamSettingsChangeListener) {
        this.divElement.removeEventListener("ml-settingschange", listener as any)
    }

    getStreamSettings(): StreamSettings {
        const settings = defaultStreamSettings()

        settings.bitrate = parseInt(this.bitrate.getValue())
        settings.packetSize = parseInt(this.packetSize.getValue())
        settings.fps = parseInt(this.fps.getValue())
        if (this.videoSizeEnabled.isChecked()) {
            settings.videoSize = {
                width: parseInt(this.videoSizeWidth.getValue()),
                height: parseInt(this.videoSizeHeight.getValue())
            }
        }
        settings.controllerConfig.invertAB = this.controllerInvertAB.isChecked()
        settings.controllerConfig.invertXY = this.controllerInvertXY.isChecked()

        return settings
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}
import { StreamControllerButton, StreamMouseButton } from "../api_bindings.js"

// https://w3c.github.io/gamepad/#remapping
const STANDARD_BUTTONS = [
    // TODO: a and b buttons might be switched (this is for nintendo)?
    StreamControllerButton.BUTTON_B,
    StreamControllerButton.BUTTON_A,
    StreamControllerButton.BUTTON_Y,
    StreamControllerButton.BUTTON_X,
    StreamControllerButton.BUTTON_LB,
    StreamControllerButton.BUTTON_RB,
    // TODO: we don't have 2 back buttons?
    StreamControllerButton.BUTTON_LB,
    StreamControllerButton.BUTTON_RB,
    // TODO: what is play and other
    StreamControllerButton.BUTTON_PLAY,
    StreamControllerButton.BUTTON_BACK,
    StreamControllerButton.BUTTON_LS_CLK,
    StreamControllerButton.BUTTON_RS_CLK,
    StreamControllerButton.BUTTON_UP,
    StreamControllerButton.BUTTON_DOWN,
    StreamControllerButton.BUTTON_LEFT,
    StreamControllerButton.BUTTON_RIGHT,
    StreamControllerButton.BUTTON_SPECIAL,
]

export function convertStandardButton(buttonIndex: number): number | null {
    return STANDARD_BUTTONS[buttonIndex] ?? null
}

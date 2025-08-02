import { ASSETS } from "../common.js"
import { Component, ListComponent } from "./component.js"

const ERROR_REMOVAL_TIME_MS = 10000

const errorListElement = document.getElementById("error-list")
const errorListComponent = new ListComponent<ErrorComponent>([], { listElementClasses: ["error-list"], componentDivClasses: ["error-element"] })
if (errorListElement) {
    errorListComponent.mount(errorListElement)
}

let alertedErrorListNotFound = false

export function showErrorPopup(message: string, fatal: boolean = false) {
    console.error(message)

    if (!errorListElement) {
        if (!alertedErrorListNotFound) {
            alert("couldn't find the error element")
            alertedErrorListNotFound = true
        }
        alert(message)
        return;
    }

    let error
    if (fatal) {
        error = new ErrorComponent(message, ASSETS.ERROR_IMAGE)
    } else {
        error = new ErrorComponent(message, ASSETS.WARN_IMAGE)
    }

    errorListComponent.append(error)

    setTimeout(() => {
        errorListComponent.removeValue(error)
    }, ERROR_REMOVAL_TIME_MS)
}

class ErrorComponent implements Component {
    private messageElement: HTMLElement = document.createElement("p")
    private imageElement: HTMLImageElement = document.createElement("img")

    constructor(message: string, image: string) {
        this.messageElement.innerText = message
        this.messageElement.classList.add("error-message")

        this.imageElement.src = image
        this.imageElement.classList.add("error-image")
    }

    mount(parent: Element): void {
        parent.appendChild(this.imageElement)
        parent.appendChild(this.messageElement)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.imageElement)
        parent.removeChild(this.messageElement)
    }
}
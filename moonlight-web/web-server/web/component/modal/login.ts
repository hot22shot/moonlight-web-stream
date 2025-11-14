import { InputComponent } from "../input.js"
import { FormModal } from "./form.js"

export type UserAuth = {
    name: string,
    password: string
}

export class ApiUserPasswordPrompt extends FormModal<UserAuth> {

    private text: HTMLElement = document.createElement("h3")

    private name: InputComponent
    private password: InputComponent
    private passwordFile: InputComponent

    constructor() {
        super()

        this.text.innerText = "Login"

        this.name = new InputComponent("ml-api-name", "text", "Username", {
            formRequired: true
        })

        this.password = new InputComponent("ml-api-password", "password", "Password", {
            formRequired: true
        })
        this.passwordFile = new InputComponent("ml-api-password-file", "file", "Password as File", { accept: ".txt" })
    }

    reset(): void {
        this.name.reset()
        this.password.reset()
        this.passwordFile.reset()
    }
    submit(): UserAuth | null {
        const name = this.name.getValue()
        const password = this.password.getValue()

        if (name && password) {
            return { name, password }
        } else {
            return null
        }
    }

    onFinish(abort: AbortSignal): Promise<UserAuth | null> {
        const abortController = new AbortController()
        abort.addEventListener("abort", abortController.abort.bind(abortController))

        return new Promise((resolve, reject) => {
            this.passwordFile.addChangeListener(() => {
                const files = this.passwordFile.getFiles()
                if (files && files.length >= 1) {
                    const file = files[0]

                    file.text().then((passwordContents) => {
                        abortController.abort()

                        // Remove carriage return and new line
                        const password = passwordContents
                            .replace(/\r/g, "")
                            .replace(/\n/g, "")

                        const name = this.name.getValue()

                        resolve({
                            name,
                            password
                        })
                    })
                }
            }, { signal: abortController.signal })

            super.onFinish(abortController.signal).then((data) => {
                abortController.abort()
                resolve(data)
            }, (data) => {
                abortController.abort()
                reject(data)
            })
        })
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.text)

        this.name.mount(form)

        this.password.mount(form)
        this.passwordFile.mount(form)
    }
}

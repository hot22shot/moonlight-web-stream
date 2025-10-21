import { InputComponent } from "../input.js"
import { FormModal } from "./form.js"

export type UserAuth = {
    username: string,
    password: string
}

export class ApiUserPasswordPrompt extends FormModal<UserAuth> {

    private text: HTMLElement = document.createElement("h3")

    private username: InputComponent
    private password: InputComponent
    private passwordFile: InputComponent

    constructor() {
        super()

        this.text.innerText = "Login"

        this.username = new InputComponent("ml-api-username", "text", "Username")

        this.password = new InputComponent("ml-api-password", "password", "Password")
        this.passwordFile = new InputComponent("ml-api-password-file", "file", "Password as File", { accept: ".txt" })
    }

    reset(): void {
        this.username.reset()
        this.password.reset()
        this.passwordFile.reset()
    }
    submit(): UserAuth | null {
        const username = this.username.getValue()
        const password = this.password.getValue()

        if (username && password) {
            return { username, password }
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

                        const username = this.username.getValue()

                        resolve({
                            username,
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

        this.username.mount(form)

        this.password.mount(form)
        this.passwordFile.mount(form)
    }
}

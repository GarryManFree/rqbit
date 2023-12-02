import { StrictMode } from "react";
import ReactDOM from 'react-dom/client';
import { RqbitWebUI, APIContext } from "./rqbit-web";
import { API } from "./http-api";

ReactDOM.createRoot(document.getElementById('app') as HTMLInputElement).render(
    <StrictMode>
        <APIContext.Provider value={API}>
            <RqbitWebUI />
        </APIContext.Provider>
    </StrictMode>
);
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/poppins/300.css";
import "@fontsource/poppins/400.css";
import "@fontsource/poppins/500.css";
import "@fontsource/poppins/600.css";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

// Reveal the document only after React has rendered the first frame.
// Pairs with the `visibility: hidden` rule in index.html and removes the
// flash of unstyled content during launch.
requestAnimationFrame(() => {
  document.documentElement.style.visibility = "visible";
});

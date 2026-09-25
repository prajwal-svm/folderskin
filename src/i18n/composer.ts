/**
 * The composer's English, which comes with the composer's own chunk rather than the app's first
 * one: every file that uses a `composer.*` key imports this for its side effect (a test checks),
 * so the words are there before anything of the composer draws, and wherever that file ends up.
 */
import composer from "../locales/en/composer.json";
import { i18n } from "./index";

i18n.addEnglish({ composer });

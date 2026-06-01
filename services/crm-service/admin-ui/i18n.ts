import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import enTranslation from './locales/en.json'

export const en = { translation: enTranslation }

i18n.use(initReactI18next).init({
  lng: 'en',
  interpolation: { escapeValue: false },
  resources: { en },
})

export default i18n

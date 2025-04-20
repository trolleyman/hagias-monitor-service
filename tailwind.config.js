/** @type {import('tailwindcss').Config} */
module.exports = {
    content: [
        "./templates/**/*.{html,tera}",
    ],
    theme: {
        extend: {
            colors: {
                primary: {
                    bg: '#1a1a1a',
                    text: '#ffffff',
                },
                secondary: {
                    bg: '#2d2d2d',
                    text: '#b3b3b3',
                },
                accent: {
                    DEFAULT: '#4a90e2',
                    hover: '#3a7bc8',
                },
            },
        },
    },
}

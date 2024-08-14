// Click to copy text
function clickToCopy(e) {
	const el = e.target
	if (!el) return
	const text = el.getAttribute('data-copy') || el.innerText
	navigator.clipboard.writeText(text)
	el.classList.add('copy-blink')
	setTimeout(() => el.classList.remove('copy-blink'), 1000)
}

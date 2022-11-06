const ELEMENTS = {};
const ELEMENT_IDS = ["input-time", "input-watts", "conversion-watts", "conversion-time"];
const ELEMENT_NAMES = ["inputTime", "inputWatts", "conversionWatts", "conversionTime"]
const TIME_PATTTERN = /^(?:(?<minutes>\d+)\:)?(?<seconds>\d+)$/;
const OLD_VALUES = new Map(ELEMENT_IDS.map(id => [id, ""]));

window.onload = () => {
    let keys = Object.fromEntries(ELEMENT_IDS.map((id, index) =>
        [ELEMENT_NAMES[index], document.getElementById(id)]
    ));

    Object.assign(ELEMENTS, keys);

    ELEMENTS.inputTime.addEventListener("change", handleTimeChanged);
    ELEMENTS.inputWatts.addEventListener("change", handleWattsChanged);
    ELEMENTS.conversionWatts.addEventListener("change", handleWattsChanged);
};

function handleTimeChanged(event) {
    if (event.target.value == "") return;

    let matches = TIME_PATTTERN.exec(event.target.value);

    if (matches == null) {
        event.target.value = OLD_VALUES.get(event.target.id);
        return;
    }

    let minutes = Number(matches.groups.minutes || 0);
    let seconds = Math.min(59, Number(matches.groups.seconds || 0));

    let newValue = formatTime(minutes, seconds);
    event.target.value = newValue;
    OLD_VALUES.set(event.target.id, newValue);

    handleConversion();
}

function handleWattsChanged(event) {
    if (event.target.value == "") return;

    if (!/\d+/.test(event.target.value)) {
        event.target.value = OLD_VALUES.get(event.target.id);
        return;
    }

    OLD_VALUES.set(event.target.id, event.target.value);

    handleConversion();
}

function handleConversion() {
    if (Object.values(ELEMENTS).filter(element => element.id != "conversion-time").some(element => element.value == "")) {
        ELEMENTS.conversionTime.textContent = "xx:xx";
        return;
    }

    let { minutes, seconds } = TIME_PATTTERN.exec(ELEMENTS.inputTime.value).groups;
    let totalSeconds = Number(minutes) * 60 + Number(seconds);
    let convertedTotalSeconds = Math.floor((totalSeconds * Number(ELEMENTS.inputWatts.value)) / Number(ELEMENTS.conversionWatts.value));

    let convertedMinutes = Math.floor(convertedTotalSeconds / 60);
    let convertedSeconds = convertedTotalSeconds % 60;

    ELEMENTS.conversionTime.textContent = formatTime(convertedMinutes, convertedSeconds);
}

function formatTime(minutes, seconds) {
    let secondsPadding = seconds < 10 ? "0" : "";
    return `${minutes}:${secondsPadding}${seconds}`;
}

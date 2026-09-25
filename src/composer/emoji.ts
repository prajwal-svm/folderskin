/**
 * The emoji picker's choices: a few hundred that make good folder icons, in groups, each with
 * the words people would search for it by. Any other emoji can still be typed or pasted into
 * the picker's field (or a text layer), since the system's emoji font draws them all.
 */

export type EmojiGroup = { id: string; icon: string; items: [char: string, words: string][] };

export const EMOJI: EmojiGroup[] = [
  {
    id: "faces",
    icon: "😀",
    items: [
      ["😀", "grin happy smile"], ["😄", "smile happy joy"], ["😂", "laugh tears funny lol"], ["😊", "blush smile happy"],
      ["😍", "love heart eyes crush"], ["🥰", "love hearts adore"], ["😎", "cool sunglasses"], ["🤓", "nerd geek glasses study"],
      ["🤔", "think thinking hmm"], ["😴", "sleep tired zzz"], ["🥳", "party celebrate birthday"], ["🤩", "star struck wow"],
      ["😇", "angel halo innocent"], ["😜", "wink silly tongue"], ["🤪", "crazy zany wild"], ["😱", "scream shock horror"],
      ["😤", "steam proud angry"], ["😡", "angry mad rage"], ["🥶", "cold freezing winter"], ["🥵", "hot heat summer"],
      ["🤯", "mind blown explode"], ["😬", "grimace awkward"], ["🙃", "upside down silly"], ["😌", "relieved calm peace"],
      ["🤗", "hug hugging"], ["🤫", "shush secret quiet"], ["🤐", "zip secret private"], ["😷", "mask sick health"],
      ["🤖", "robot bot ai tech"], ["👻", "ghost boo halloween"], ["💀", "skull dead spooky"], ["👽", "alien space ufo"],
      ["👾", "game invader arcade retro"], ["🤡", "clown circus"], ["💩", "poop funny"], ["😺", "cat smile happy"],
      ["👋", "wave hello hi bye"], ["👍", "thumbs up yes ok good"], ["👎", "thumbs down no bad"], ["👏", "clap applause bravo"],
      ["🙌", "hands raise celebrate hooray"], ["🙏", "pray thanks please"], ["💪", "muscle strong gym fitness"], ["✌️", "peace victory"],
      ["🤞", "fingers crossed luck hope"], ["👌", "ok perfect"], ["🤝", "handshake deal agreement"], ["✍️", "write writing sign"],
      ["👀", "eyes look watch see"], ["🧠", "brain smart think mind"], ["👶", "baby child newborn"], ["🧑‍💻", "coder developer programmer laptop"],
      ["🧑‍🎨", "artist painter art"], ["🧑‍🍳", "cook chef kitchen"], ["🧑‍🎓", "student graduate school"], ["🧑‍🚀", "astronaut space"],
    ],
  },
  {
    id: "symbols",
    icon: "❤️",
    items: [
      ["❤️", "heart love red"], ["🧡", "heart orange"], ["💛", "heart yellow"], ["💚", "heart green"], ["💙", "heart blue"],
      ["💜", "heart purple"], ["🖤", "heart black"], ["🤍", "heart white"], ["🤎", "heart brown"], ["💖", "heart sparkle love"],
      ["💘", "heart arrow cupid"], ["💝", "heart gift ribbon"], ["💕", "hearts love two"], ["💔", "broken heart"], ["⭐", "star favourite"],
      ["🌟", "star glow shine"], ["✨", "sparkles magic new shine"], ["⚡", "lightning bolt power energy fast"], ["🔥", "fire hot lit flame"],
      ["💥", "boom bang collision"], ["💫", "dizzy star"], ["💯", "hundred perfect score"], ["✅", "check done tick complete"],
      ["❌", "cross no wrong delete"], ["❗", "exclamation important alert"], ["❓", "question help"], ["⚠️", "warning caution"],
      ["🚫", "no forbidden prohibited"], ["♻️", "recycle eco green"], ["☮️", "peace"], ["☯️", "yin yang balance"],
      ["🔔", "bell notification alert"], ["🎵", "music note song"], ["🎶", "music notes melody"], ["➕", "plus add"],
      ["✔️", "check mark tick"], ["🔒", "lock locked private secure"], ["🔓", "unlock open"], ["🔑", "key password access"],
      ["🏷️", "label tag price"], ["📌", "pin pushpin important"], ["📍", "location pin map place"], ["🔖", "bookmark"],
      ["💡", "idea light bulb"], ["♾️", "infinity forever"], ["🆕", "new"], ["🆒", "cool"], ["🔝", "top"],
      ["🔴", "red circle"], ["🟠", "orange circle"], ["🟡", "yellow circle"], ["🟢", "green circle"], ["🔵", "blue circle"],
      ["🟣", "purple circle"], ["⚫", "black circle"], ["⚪", "white circle"], ["🟥", "red square"], ["🟧", "orange square"],
      ["🟨", "yellow square"], ["🟩", "green square"], ["🟦", "blue square"], ["🟪", "purple square"], ["⬛", "black square"],
    ],
  },
  {
    id: "nature",
    icon: "🐶",
    items: [
      ["🐶", "dog puppy pet"], ["🐱", "cat kitten pet"], ["🐭", "mouse"], ["🐹", "hamster pet"], ["🐰", "rabbit bunny easter"],
      ["🦊", "fox"], ["🐻", "bear teddy"], ["🐼", "panda"], ["🐨", "koala"], ["🐯", "tiger"], ["🦁", "lion king"],
      ["🐮", "cow farm"], ["🐷", "pig farm"], ["🐸", "frog"], ["🐵", "monkey"], ["🐔", "chicken hen"], ["🐧", "penguin"],
      ["🐦", "bird"], ["🦆", "duck"], ["🦉", "owl night wise"], ["🦄", "unicorn magic"], ["🐝", "bee honey"], ["🦋", "butterfly"],
      ["🐞", "ladybug bug beetle"], ["🐢", "turtle slow"], ["🐍", "snake python"], ["🦖", "dinosaur t rex"], ["🐙", "octopus"],
      ["🦀", "crab"], ["🐠", "fish tropical"], ["🐬", "dolphin"], ["🐳", "whale"], ["🦈", "shark"], ["🐘", "elephant"],
      ["🦒", "giraffe"], ["🦓", "zebra"], ["🐎", "horse"], ["🐑", "sheep"], ["🦔", "hedgehog"], ["🐾", "paw prints pet"],
      ["🌵", "cactus desert"], ["🌲", "tree evergreen pine"], ["🌳", "tree"], ["🌴", "palm tree beach tropical"], ["🌱", "seedling plant grow"],
      ["🌿", "herb leaf plant"], ["🍀", "clover luck"], ["🍁", "maple leaf autumn fall canada"], ["🍂", "leaves autumn fall"],
      ["🍄", "mushroom"], ["🌷", "tulip flower spring"], ["🌹", "rose flower love"], ["🌺", "hibiscus flower"], ["🌸", "cherry blossom flower spring"],
      ["🌼", "blossom flower"], ["🌻", "sunflower flower summer"], ["💐", "bouquet flowers"], ["🌞", "sun smile sunny"], ["🌙", "moon night"],
      ["🌈", "rainbow pride"], ["☁️", "cloud weather"], ["⛅", "sun cloud weather"], ["🌧️", "rain weather"], ["⛈️", "storm thunder"],
      ["❄️", "snowflake snow winter cold"], ["☃️", "snowman winter"], ["🌊", "wave ocean sea surf"], ["💧", "droplet water"],
      ["🌍", "earth world globe planet"], ["🌋", "volcano"],
    ],
  },
  {
    id: "food",
    icon: "🍓",
    items: [
      ["🍎", "apple red fruit"], ["🍐", "pear fruit"], ["🍊", "orange tangerine fruit"], ["🍋", "lemon fruit"], ["🍌", "banana fruit"],
      ["🍉", "watermelon fruit summer"], ["🍇", "grapes fruit"], ["🍓", "strawberry fruit"], ["🫐", "blueberries fruit"], ["🍒", "cherries fruit"],
      ["🍑", "peach fruit"], ["🥭", "mango fruit"], ["🍍", "pineapple fruit"], ["🥥", "coconut"], ["🥑", "avocado"], ["🍅", "tomato"],
      ["🥕", "carrot"], ["🌽", "corn"], ["🌶️", "chili pepper hot spicy"], ["🥦", "broccoli"], ["🍞", "bread"], ["🥐", "croissant"],
      ["🥨", "pretzel"], ["🧀", "cheese"], ["🥚", "egg"], ["🍳", "cooking egg pan breakfast"], ["🥞", "pancakes breakfast"],
      ["🍔", "burger hamburger"], ["🍟", "fries"], ["🍕", "pizza"], ["🌭", "hot dog"], ["🌮", "taco"], ["🌯", "burrito wrap"],
      ["🥗", "salad healthy"], ["🍝", "spaghetti pasta"], ["🍜", "ramen noodles"], ["🍣", "sushi"], ["🍱", "bento"], ["🍤", "shrimp tempura"],
      ["🍙", "rice ball onigiri"], ["🍩", "doughnut donut"], ["🍪", "cookie"], ["🎂", "birthday cake"], ["🍰", "cake slice"],
      ["🧁", "cupcake"], ["🍫", "chocolate"], ["🍬", "candy sweet"], ["🍭", "lollipop candy"], ["🍦", "ice cream"], ["🍿", "popcorn movie"],
      ["☕", "coffee tea hot"], ["🍵", "tea matcha"], ["🧋", "bubble tea boba"], ["🥤", "drink soda cup"], ["🍺", "beer"],
      ["🍷", "wine"], ["🍸", "cocktail martini"], ["🍹", "tropical drink cocktail"], ["🥂", "cheers champagne toast"],
    ],
  },
  {
    id: "travel",
    icon: "✈️",
    items: [
      ["✈️", "plane flight travel trip"], ["🚀", "rocket launch space"], ["🛸", "ufo flying saucer"], ["🚗", "car drive"], ["🚕", "taxi"],
      ["🚌", "bus"], ["🚲", "bike bicycle cycling"], ["🛵", "scooter"], ["🏍️", "motorcycle"], ["🚂", "train"], ["🚢", "ship cruise"],
      ["⛵", "sailboat sailing"], ["🚁", "helicopter"], ["🗺️", "map travel"], ["🧭", "compass explore"], ["🏔️", "mountain snow"],
      ["⛰️", "mountain hiking"], ["🏕️", "camping tent"], ["🏖️", "beach umbrella holiday vacation"], ["🏝️", "island tropical"],
      ["🏜️", "desert"], ["🗽", "statue of liberty new york"], ["🗼", "tokyo tower"], ["🏰", "castle"], ["🏯", "japanese castle"],
      ["🏠", "house home"], ["🏡", "home garden house"], ["🏢", "office building work"], ["🏫", "school"], ["🏥", "hospital"],
      ["🏦", "bank money"], ["🏨", "hotel"], ["⛪", "church"], ["🕌", "mosque"], ["🌆", "city dusk"], ["🌃", "night city"],
      ["🌉", "bridge night"], ["🎡", "ferris wheel fair"], ["🎢", "roller coaster"], ["🎠", "carousel"], ["⛱️", "parasol beach"],
      ["🧳", "luggage suitcase travel"], ["🛎️", "bell hotel service"], ["🌐", "globe web internet"],
    ],
  },
  {
    id: "activities",
    icon: "🎨",
    items: [
      ["⚽", "soccer football"], ["🏀", "basketball"], ["🏈", "american football"], ["⚾", "baseball"], ["🎾", "tennis"],
      ["🏐", "volleyball"], ["🏉", "rugby"], ["🎱", "pool billiards"], ["🏓", "ping pong table tennis"], ["🏸", "badminton"],
      ["🥊", "boxing"], ["⛳", "golf"], ["🏹", "archery bow"], ["🎣", "fishing"], ["🤿", "diving snorkel"], ["🎿", "ski skiing"],
      ["🏂", "snowboard"], ["🛹", "skateboard"], ["🏄", "surf surfing"], ["🏊", "swim swimming"], ["🚴", "cycling bike"],
      ["🧘", "yoga meditation"], ["🏋️", "weights gym lifting"], ["🎮", "video game controller gaming"], ["🕹️", "joystick arcade"],
      ["🎲", "dice game"], ["🧩", "puzzle piece"], ["♟️", "chess"], ["🎯", "target goal bullseye"], ["🎳", "bowling"],
      ["🎨", "art palette paint"], ["🖌️", "paintbrush art"], ["🖍️", "crayon draw"], ["🎭", "theatre drama masks"], ["🎬", "film movie clapper"],
      ["🎤", "microphone sing karaoke"], ["🎧", "headphones music podcast"], ["🎸", "guitar music rock"], ["🎹", "piano keyboard music"],
      ["🥁", "drum music"], ["🎺", "trumpet music"], ["🎻", "violin music"], ["🎷", "saxophone jazz"], ["📷", "camera photo"],
      ["📸", "camera flash photos"], ["🎥", "video camera film"], ["🎁", "gift present"], ["🎈", "balloon party"], ["🎉", "party popper celebrate"],
      ["🎊", "confetti ball celebrate"], ["🎄", "christmas tree"], ["🎃", "pumpkin halloween"], ["🏆", "trophy winner award"],
      ["🥇", "gold medal first"], ["🏅", "medal sports"], ["🎟️", "ticket event"],
    ],
  },
  {
    id: "objects",
    icon: "💼",
    items: [
      ["📁", "folder file"], ["📂", "open folder file"], ["🗂️", "dividers index"], ["📄", "document page"], ["📃", "page curl"],
      ["📑", "tabs bookmark"], ["📊", "chart bar stats"], ["📈", "chart up growth"], ["📉", "chart down"], ["🗒️", "notepad notes"],
      ["📝", "memo notes write"], ["✏️", "pencil edit"], ["🖊️", "pen"], ["🖋️", "fountain pen"], ["📎", "paperclip attach"],
      ["📐", "triangle ruler design"], ["📏", "ruler measure"], ["✂️", "scissors cut"], ["🗃️", "card box archive"], ["🗄️", "cabinet files archive"],
      ["🗑️", "trash bin delete"], ["📅", "calendar date"], ["🗓️", "calendar schedule"], ["📇", "card index contacts"], ["📋", "clipboard"],
      ["📚", "books library study"], ["📖", "book read"], ["📓", "notebook"], ["📕", "book red"], ["📗", "book green"], ["📘", "book blue"],
      ["💼", "briefcase work business"], ["🎒", "backpack school"], ["🧾", "receipt bill taxes"], ["💰", "money bag"], ["💵", "dollar money cash"],
      ["💳", "credit card payment"], ["🪙", "coin money"], ["💎", "gem diamond jewel"], ["⚖️", "scales law justice"], ["🔧", "wrench tool fix"],
      ["🔨", "hammer tool build"], ["🛠️", "tools settings"], ["⚙️", "gear settings cog"], ["🧰", "toolbox"], ["🧲", "magnet"],
      ["🔬", "microscope science"], ["🔭", "telescope space"], ["🧪", "test tube lab science"], ["🧬", "dna science biology"], ["💊", "pill medicine"],
      ["🩺", "stethoscope doctor health"], ["💻", "laptop computer code"], ["🖥️", "desktop computer"], ["⌨️", "keyboard"], ["🖱️", "mouse computer"],
      ["📱", "phone mobile"], ["☎️", "telephone"], ["📺", "tv television"], ["📻", "radio"], ["🎙️", "studio microphone podcast"],
      ["💾", "floppy disk save"], ["💿", "cd disc"], ["🔋", "battery"], ["🔌", "plug electric"], ["🔦", "flashlight torch"],
      ["🕯️", "candle"], ["🛒", "shopping cart"], ["📦", "package box parcel"], ["📫", "mailbox"], ["✉️", "envelope mail letter"],
      ["📧", "email"], ["🔐", "locked key secure"], ["🧸", "teddy bear toy"], ["🪴", "potted plant"], ["🛋️", "couch sofa home"],
      ["🛏️", "bed sleep"], ["🧹", "broom clean"], ["🧺", "basket laundry picnic"], ["👕", "t shirt clothes"], ["👗", "dress fashion"],
      ["👟", "sneaker shoe running"], ["👑", "crown king queen"], ["👓", "glasses"], ["🕶️", "sunglasses"], ["⌚", "watch time"],
      ["⏰", "alarm clock"], ["⏳", "hourglass time"],
    ],
  },
  {
    id: "flags",
    icon: "🏳️‍🌈",
    items: [
      ["🏁", "chequered flag finish race"], ["🏳️", "white flag"], ["🏴", "black flag"], ["🏳️‍🌈", "rainbow flag pride"], ["🏴‍☠️", "pirate flag"],
      ["🇺🇸", "usa united states america"], ["🇬🇧", "uk united kingdom britain"], ["🇨🇦", "canada"], ["🇮🇳", "india"], ["🇯🇵", "japan"],
      ["🇫🇷", "france"], ["🇩🇪", "germany"], ["🇮🇹", "italy"], ["🇪🇸", "spain"], ["🇧🇷", "brazil"], ["🇲🇽", "mexico"],
      ["🇦🇺", "australia"], ["🇨🇳", "china"], ["🇰🇷", "south korea"], ["🇳🇱", "netherlands"], ["🇸🇪", "sweden"], ["🇳🇴", "norway"],
      ["🇨🇭", "switzerland"], ["🇮🇪", "ireland"], ["🇳🇿", "new zealand"], ["🇿🇦", "south africa"], ["🇪🇺", "european union europe"],
    ],
  },
];

/** Every emoji whose words start with or contain `query`, in group order. */
export function searchEmoji(query: string, limit = 120): string[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: string[] = [];
  for (const group of EMOJI)
    for (const [char, words] of group.items) {
      if (out.length >= limit) return out;
      if (words.split(" ").some((w) => w.startsWith(q)) || (q.length > 2 && words.includes(q))) out.push(char);
    }
  return out;
}
